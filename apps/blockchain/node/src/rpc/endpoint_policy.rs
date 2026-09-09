//! Storage Node エンドポイント URL の SSRF 対策ポリシー
//!
//! `storage_registerEndpoint` RPC / gossip 経由で登録された URL に対して、
//! チェーンノードは upload/get fan-out 時に認証付き POST を発行し、
//! レスポンス/エラーテキストを呼び出し元に返す。URL を検証しないと
//! `http://169.254.169.254/...` (クラウドメタデータ) や自ノード RPC
//! (`http://127.0.0.1:9944`) への SSRF が可能になる。
//!
//! ## ポリシー
//!
//! - link-local (169.254.0.0/16, fe80::/10) / multicast / unspecified /
//!   broadcast は **常に拒否**
//! - loopback / RFC1918 / CGNAT / IPv6 ULA は **dev (`--dev` / `--chain local`)
//!   のときのみ許可** (dev は storage-node が 127.0.0.1:3030-3034 で動くため)
//! - 自ノードの RPC ポート宛 URL は **常に拒否** (RPC ループバック SSRF 防止)
//! - `.onion` / `.i2p` ホストは IP 検査をスキップ (Tor/I2P 経由が正規経路で、
//!   システム DNS では解決できない)
//! - その他のドメイン名は登録時に DNS 解決し、解決先 IP 全てを同じ規則で検査
//!   (DNS rebinding を完全には防げないが、登録時点の注入は弾く)

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// DNS 解決のタイムアウト (gossip ループを長時間ブロックしないため)
const DNS_LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// エンドポイント URL 検証ポリシー
///
/// `service.rs` でチェーンスペックの `ChainType` と RPC listen ポートから
/// 構築され、RPC ハンドラ (`Storage`) と gossip サービスの両方に渡される。
#[derive(Clone, Debug)]
pub struct EndpointPolicy {
    /// dev / local チェーンでは loopback / private IP のエンドポイントを許可する
    pub allow_private: bool,
    /// 自ノードが listen している RPC ポート (このポート宛の URL は常に拒否)
    pub own_rpc_ports: Vec<u16>,
}

impl EndpointPolicy {
    /// IPv4-mapped IPv6 (`::ffff:a.b.c.d`) を IPv4 に正規化
    fn canonical_ip(ip: IpAddr) -> IpAddr {
        match ip {
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => IpAddr::V4(v4),
                None => IpAddr::V6(v6),
            },
            v4 => v4,
        }
    }

    /// IPv4 が RFC1918 / CGNAT / loopback (= dev のみ許可) か
    fn is_private_v4(v4: &Ipv4Addr) -> bool {
        let o = v4.octets();
        v4.is_loopback()
            || v4.is_private()
            // CGNAT 100.64.0.0/10
            || (o[0] == 100 && (o[1] & 0xc0) == 64)
    }

    /// IPv6 が loopback / ULA (fc00::/7) (= dev のみ許可) か
    fn is_private_v6(v6: &Ipv6Addr) -> bool {
        v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00
    }

    /// IPv6 link-local (fe80::/10) か (std の is_unicast_link_local は
    /// stable で使えないバージョンがあるため手動実装)
    fn is_link_local_v6(v6: &Ipv6Addr) -> bool {
        (v6.segments()[0] & 0xffc0) == 0xfe80
    }

    /// 単一 IP アドレスをポリシーに照らして検査
    pub fn check_ip(&self, ip: &IpAddr) -> Result<(), String> {
        match Self::canonical_ip(*ip) {
            IpAddr::V4(v4) => {
                // 常に拒否: link-local (クラウドメタデータ 169.254.169.254 含む) /
                // multicast / unspecified / broadcast
                if v4.is_link_local()
                    || v4.is_multicast()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
                {
                    return Err(format!(
                        "Forbidden endpoint IP {} (link-local/multicast/unspecified/broadcast)",
                        v4
                    ));
                }
                if !self.allow_private && Self::is_private_v4(&v4) {
                    return Err(format!(
                        "Private/loopback endpoint IP {} is not allowed on this chain",
                        v4
                    ));
                }
            }
            IpAddr::V6(v6) => {
                if Self::is_link_local_v6(&v6) || v6.is_multicast() || v6.is_unspecified() {
                    return Err(format!(
                        "Forbidden endpoint IP {} (link-local/multicast/unspecified)",
                        v6
                    ));
                }
                if !self.allow_private && Self::is_private_v6(&v6) {
                    return Err(format!(
                        "Private/loopback endpoint IP {} is not allowed on this chain",
                        v6
                    ));
                }
            }
        }
        Ok(())
    }

    /// 同期検証: スキーム / ポート / リテラル IP / localhost。
    /// DNS 解決は行わない (オンチェーン URL の fan-out 時など、リクエスト毎に
    /// 呼ばれるホットパスで使用)。
    pub fn validate_url_sync(&self, url_str: &str) -> Result<url::Url, String> {
        let parsed =
            url::Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;

        match parsed.scheme() {
            "http" | "https" => {}
            other => return Err(format!("Invalid URL scheme '{}': must be http or https", other)),
        }

        let port = parsed
            .port_or_known_default()
            .ok_or_else(|| "URL has no port".to_string())?;
        // 自ノード RPC ポート宛は常に拒否 (どのホストでも: 自 RPC への SSRF ループ防止。
        // storage-node は別ポート (:3030 系) で listen する)
        if self.own_rpc_ports.contains(&port) {
            return Err(format!(
                "Endpoint port {} is this chain node's own RPC port",
                port
            ));
        }

        match parsed.host() {
            None => return Err("URL has no host".to_string()),
            Some(url::Host::Ipv4(v4)) => self.check_ip(&IpAddr::V4(v4))?,
            Some(url::Host::Ipv6(v6)) => self.check_ip(&IpAddr::V6(v6))?,
            Some(url::Host::Domain(domain)) => {
                // localhost は loopback と同等に扱う
                if domain.eq_ignore_ascii_case("localhost") && !self.allow_private {
                    return Err(
                        "localhost endpoint is not allowed on this chain".to_string()
                    );
                }
                // その他のドメインは async 版 validate_url で DNS 検査する
            }
        }

        Ok(parsed)
    }

    /// 完全検証 (登録時に使用): 同期検証 + ドメイン名の DNS 解決結果も検査。
    ///
    /// `.onion` / `.i2p` は Tor/I2P 経由が正規経路でシステム DNS では解決
    /// できないためスキップする。それ以外のドメインは解決に失敗したら拒否
    /// (検証できない宛先には fan-out しない)。
    pub async fn validate_url(&self, url_str: &str) -> Result<(), String> {
        let parsed = self.validate_url_sync(url_str)?;

        let domain = match parsed.host() {
            Some(url::Host::Domain(d)) => d.to_ascii_lowercase(),
            // リテラル IP / localhost は sync 検証で完結
            _ => return Ok(()),
        };

        if domain == "localhost" || domain.ends_with(".onion") || domain.ends_with(".i2p") {
            return Ok(());
        }

        // SAFETY: validate_url_sync が port_or_known_default の存在を保証済み
        let port = parsed.port_or_known_default().unwrap_or(80);
        let addrs = tokio::time::timeout(
            DNS_LOOKUP_TIMEOUT,
            tokio::net::lookup_host((domain.as_str(), port)),
        )
        .await
        .map_err(|_| format!("DNS lookup for '{}' timed out", domain))?
        .map_err(|e| format!("DNS lookup for '{}' failed: {}", domain, e))?;

        let mut resolved_any = false;
        for sock_addr in addrs {
            resolved_any = true;
            self.check_ip(&sock_addr.ip())
                .map_err(|e| format!("Host '{}' resolves to forbidden address: {}", domain, e))?;
        }
        if !resolved_any {
            return Err(format!("Host '{}' resolved to no addresses", domain));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev_policy() -> EndpointPolicy {
        EndpointPolicy { allow_private: true, own_rpc_ports: vec![9944] }
    }

    fn live_policy() -> EndpointPolicy {
        EndpointPolicy { allow_private: false, own_rpc_ports: vec![9944] }
    }

    // クラウドメタデータ IP (link-local) は dev でも常に拒否
    #[test]
    fn metadata_ip_rejected_always() {
        assert!(dev_policy()
            .validate_url_sync("http://169.254.169.254/latest/meta-data")
            .is_err());
        assert!(live_policy()
            .validate_url_sync("http://169.254.169.254/latest/meta-data")
            .is_err());
        // IPv4-mapped IPv6 でのバイパスも拒否
        assert!(dev_policy()
            .validate_url_sync("http://[::ffff:169.254.169.254]:3030")
            .is_err());
        // IPv6 link-local も常に拒否
        assert!(dev_policy().validate_url_sync("http://[fe80::1]:3030").is_err());
    }

    // unspecified / multicast / broadcast は常に拒否
    #[test]
    fn special_ips_rejected_always() {
        for url in [
            "http://0.0.0.0:3030",
            "http://224.0.0.1:3030",
            "http://255.255.255.255:3030",
            "http://[::]:3030",
            "http://[ff02::1]:3030",
        ] {
            assert!(dev_policy().validate_url_sync(url).is_err(), "should reject {}", url);
            assert!(live_policy().validate_url_sync(url).is_err(), "should reject {}", url);
        }
    }

    // loopback は dev では許可 (storage-node が 127.0.0.1:3030-3034 で動く)
    #[test]
    fn loopback_allowed_in_dev() {
        assert!(dev_policy().validate_url_sync("http://127.0.0.1:3030").is_ok());
        assert!(dev_policy().validate_url_sync("http://localhost:3031").is_ok());
        assert!(dev_policy().validate_url_sync("http://[::1]:3032").is_ok());
    }

    // loopback / RFC1918 / CGNAT は非 dev では拒否
    #[test]
    fn private_rejected_in_live() {
        for url in [
            "http://127.0.0.1:3030",
            "http://localhost:3030",
            "http://[::1]:3030",
            "http://10.0.0.1:3030",
            "http://172.16.0.1:3030",
            "http://192.168.1.1:3030",
            "http://100.64.0.1:3030",
            "http://[fd00::1]:3030",
        ] {
            assert!(live_policy().validate_url_sync(url).is_err(), "should reject {}", url);
        }
    }

    // 自ノードの RPC ポート宛は dev でも常に拒否
    #[test]
    fn own_rpc_port_rejected_always() {
        assert!(dev_policy().validate_url_sync("http://127.0.0.1:9944").is_err());
        assert!(live_policy().validate_url_sync("http://203.0.113.7:9944").is_err());
        assert!(dev_policy().validate_url_sync("http://some-host.example:9944").is_err());
    }

    // スキーム検証
    #[test]
    fn non_http_scheme_rejected() {
        assert!(dev_policy().validate_url_sync("ftp://127.0.0.1:3030").is_err());
        assert!(dev_policy().validate_url_sync("file:///etc/passwd").is_err());
        assert!(dev_policy().validate_url_sync("not a url").is_err());
    }

    // グローバル IP / onion は許可
    #[test]
    fn public_endpoints_allowed() {
        assert!(live_policy().validate_url_sync("http://203.0.113.7:3030").is_ok());
        assert!(live_policy().validate_url_sync("https://203.0.113.7").is_ok());
        assert!(live_policy()
            .validate_url_sync("http://abcdefghijklmnop.onion:3030")
            .is_ok());
    }

    // async 版: DNS 不要パス (リテラル IP / onion) の挙動確認
    #[test]
    fn validate_url_async_no_dns_paths() {
        // リテラル IP / .onion は DNS 解決なしで完結するため block_on で安全に実行できる
        futures::executor::block_on(async {
            assert!(dev_policy().validate_url("http://127.0.0.1:3030").await.is_ok());
            assert!(live_policy().validate_url("http://127.0.0.1:3030").await.is_err());
            assert!(dev_policy()
                .validate_url("http://169.254.169.254/latest")
                .await
                .is_err());
            assert!(live_policy()
                .validate_url("http://abcdefghijklmnop.onion:3030")
                .await
                .is_ok());
            assert!(dev_policy().validate_url("http://127.0.0.1:9944").await.is_err());
        });
    }
}
