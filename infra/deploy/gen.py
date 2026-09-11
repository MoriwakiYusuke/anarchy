#!/usr/bin/env python3
"""
デプロイ構成のジェネレータ。

ホストごとに「チェーン何台・ストレージ何台・採掘するか」を引数で指定し、
compose.yml / torrc / torsocks.conf / storage.toml.tmpl / .env.example を生成する。

  # core: chain×3 (2 台が validator, 1 台が採掘) + storage×3。他ホストの bootnode になるので node-key 固定
  ./gen.py core --chains 3 --validators 2 --mine --storage 3 --capacity 10G --node-key 00…01

  # gateway: chain×1 + storage×3、core を bootnode にし、RPC を nginx 経由で公開する
  ./gen.py gateway --chains 1 --storage 3 --capacity 5G --bootnode-core --public-rpc --domain rpc.example.org --subnet 30

  # storage-only: storage×10、チェーンは別ホスト
  ./gen.py storage-only --storage 10 --capacity 2G

台数を変えたいときは引数を変えて再生成するだけ。手で YAML を触らない。
生成した compose.yml の先頭に再生成コマンドが書いてあるので、それを見れば現在の構成が分かる。
生成後の手順は README.md。

設計上の制約 (実測で判明したもの。変える前に docs/operations/deployment-multi-provider.md §5 を読むこと):
  - torsocks は全 outbound を Tor に流すため、同一ホスト宛の通信を持つプロセスを包めない
    → チェーン同士は netns コンテナの名前空間を共有し 127.0.0.1 でピアする
    → チェーンと同居する storage は包まない。チェーンが別ホストなら包む
  - 名前空間は tor ではなく netns コンテナが持つ (tor 再起動でチェーンが分岐するのを防ぐ)
  - tor の HiddenServicePort と torsocks.conf の TorAddress にホスト名は使えない (IP のみ)
  - libp2p は /onion3/ も /dns4/<onion>/ も dial できない → 跨ホストは socat が必須
  - GRANDPA は keystore の gran 鍵 + --validator + オーソリティの 2/3 以上、3 条件が要る
  - --validator と --rpc-external は併用できない
"""
import argparse, pathlib, sys, textwrap

REGISTRY = "ghcr.io/moriwakiyusuke"
IP_STORAGE_BASE = 30          # storage-N → <subnet>.(30+N)
P2P_BASE, RPC_BASE, PROM_BASE = 30332, 9943, 9614   # chain-N → +N
STORAGE_PORT_BASE = 3029     # storage-N → 3029+N  (3030..)
LIBP2P_BASE = 4000           # storage-N → 4000+N

def parse_size(s: str) -> int:
    s = s.strip().upper()
    for suf, mul in (("G", 1024**3), ("M", 1024**2)):
        if s.endswith(suf) or s.endswith(suf + "IB") or s.endswith(suf + "B"):
            return int(float(s.rstrip("IB").rstrip(suf))) * mul
    return int(s)

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("out", help="出力ディレクトリ名 (core / gateway / storage-only など)")
    ap.add_argument("--chains", type=int, default=0)
    ap.add_argument("--validators", type=int, default=0, help="GRANDPA voter にするチェーン台数 (先頭から)")
    ap.add_argument("--mine", action="store_true", help="chain-1 で採掘する")
    ap.add_argument("--storage", type=int, default=0)
    ap.add_argument("--capacity", default="10G", help="storage 1 台あたり (例: 10G, 2G, 512M)")
    ap.add_argument("--bootnode-core", action="store_true",
                    help="core の chain onion を bootnode にする (gateway 用)。socat トンネルを組み込む")
    ap.add_argument("--public-rpc", action="store_true",
                    help="chain-1 の RPC を外部に出す (--rpc-external)。validator とは併用不可")
    ap.add_argument("--domain", help="--public-rpc のとき nginx conf に使うホスト名 (例: rpc.anarchy2026.org)")
    ap.add_argument("--node-key", metavar="HEX64",
                    help="chain-1 の libp2p 秘密鍵 (hex 64 桁)。**他ホストから bootnode として参照されるホストだけ**に指定する。"
                         " 未指定なら初回起動時に生成され volume に永続化される (peer ID はログから取得)")
    ap.add_argument("--subnet", type=int, default=28, metavar="N",
                    help="compose ネットワークの第 3 オクテット (172.N.0.0/24)。既定 28")
    a = ap.parse_args()

    if a.public_rpc and not a.domain:
        sys.exit("--public-rpc には --domain が必要 (nginx conf を生成するため)")
    if a.node_key and len(a.node_key) != 64:
        sys.exit("--node-key は hex 64 桁")

    SUBNET   = f"172.{a.subnet}.0.0/24"
    IP_NETNS = f"172.{a.subnet}.0.10"
    ip_storage = lambda i: f"172.{a.subnet}.0.{IP_STORAGE_BASE + i}"

    if a.validators > a.chains:
        sys.exit("--validators は --chains 以下にすること")
    if a.public_rpc and a.validators >= 1:
        sys.exit("--public-rpc と --validators は併用できない (--rpc-external と --validator は排他)")
    if a.chains == 0 and a.storage == 0:
        sys.exit("--chains か --storage のどちらかは 1 以上にすること")

    remote_chain = a.chains == 0          # チェーンが同居しない → storage を torsocks で包む
    has_netns    = a.chains > 0           # チェーンがあれば名前空間共有が要る
    out = pathlib.Path(__file__).parent / a.out
    out.mkdir(parents=True, exist_ok=True)

    # ------------------------------------------------------------------ compose
    L = []
    W = L.append
    W(f"# {a.out} — " + " + ".join(
        [f"chain×{a.chains}" if a.chains else "",
         f"storage×{a.storage}" if a.storage else "",
         "採掘" if a.mine else ""] and [x for x in [
         f"chain×{a.chains}" if a.chains else None,
         f"storage×{a.storage}" if a.storage else None,
         "採掘" if a.mine else None] if x]))
    W("#")
    W("# ⚠️ このファイルは gen.py が生成する。手で編集せず、引数を変えて再生成すること:")
    W(f"#   ./gen.py {a.out} " + " ".join(
        ([f"--chains {a.chains}"] if a.chains else []) +
        ([f"--validators {a.validators}"] if a.validators else []) +
        (["--mine"] if a.mine else []) +
        ([f"--storage {a.storage}"] if a.storage else []) +
        ([f"--capacity {a.capacity}"] if a.storage else []) +
        (["--bootnode-core"] if a.bootnode_core else []) +
        (["--public-rpc", f"--domain {a.domain}"] if a.public_rpc else []) +
        ([f"--node-key {a.node_key}"] if a.node_key else []) +
        ([f"--subnet {a.subnet}"] if a.subnet != 28 else [])))
    W("#")
    W("# 設計の根拠は gen.py 冒頭と docs/operations/deployment-multi-provider.md §5 を参照。")
    W("")
    W("services:")

    if has_netns:
        W("  # 名前空間の保持だけを担う (tor の再起動と切り離すため)")
        W("  netns:")
        W("    image: alpine:3.20")
        W('    command: ["sleep", "infinity"]')
        W("    restart: unless-stopped")
        W("    networks:")
        W("      anarchy:")
        W(f"        ipv4_address: {IP_NETNS}")
        W("")

    W("  tor:")
    W("    build:")
    W("      context: ../../docker/tor")
    W("    image: anarchy/tor:local")
    W("    restart: unless-stopped")
    if has_netns:
        W('    network_mode: "service:netns"')
        W("    depends_on: [netns]")
    W("    volumes:")
    W("      - tor-data:/var/lib/tor")
    W("      - ./torrc:/etc/tor/torrc:ro")
    W("    healthcheck:")
    W('      test: ["CMD", "sh", "-c", "nc -z 127.0.0.1 9050"]')
    W("      interval: 5s")
    W("      timeout: 2s")
    W("      retries: 24")
    W("      start_period: 10s")
    W("")

    if a.bootnode_core:
        W("  # core の chain onion をローカル TCP に落とすトンネル。")
        W("  # libp2p は /onion3/ も /dns4/<onion>/ も dial できない (実測済み) ため必須。")
        W("  onion-proxy:")
        W("    image: alpine/socat:latest")
        W("    restart: unless-stopped")
        W('    network_mode: "service:netns"')
        W("    depends_on:")
        W("      tor:")
        W("        condition: service_healthy")
        W("    command: >-")
        W("      TCP-LISTEN:30350,bind=127.0.0.1,reuseaddr,fork")
        W("      SOCKS4A:127.0.0.1:${CORE_CHAIN_ONION:?core の chain onion を設定してください}:30333,socksport=9050")
        W("")

    for i in range(1, a.chains + 1):
        p2p, rpc, prom = P2P_BASE + i, RPC_BASE + i, PROM_BASE + i
        is_validator = i <= a.validators
        is_miner = a.mine and i == 1
        cmd = [
            "--chain=/etc/anarchy/chainspec.json",
            "--base-path=/data",
            f"--listen-addr=/ip4/0.0.0.0/tcp/{p2p}",
            f"--rpc-port={rpc}",
            f"--prometheus-port={prom}",
        ]
        if a.public_rpc and i == 1:
            cmd.append("--rpc-external")
        cmd.append("--rpc-cors=${ANARCHY_RPC_CORS:-all}" if (a.public_rpc and i == 1) else "--rpc-cors=all")
        if i == 1 and a.node_key:
            cmd.append(f"--node-key={a.node_key}")
        else:
            # 初回起動で生成され <base-path>/chains/<id>/network/ に永続化される。
            # volume を消さない限り peer ID は変わらない。
            cmd.append("--unsafe-force-node-key-generation")
        if is_validator:
            cmd.append("--validator")
        if is_miner:
            cmd += ["--mine", "--coinbase=${ANARCHY_COINBASE:?ANARCHY_COINBASE を設定してください}",
                    "--randomx-mode=light"]
        if a.bootnode_core and i == 1:
            cmd.append("--bootnodes=/ip4/127.0.0.1/tcp/30350/p2p/${CORE_CHAIN_PEER_ID:?core の chain-1 peer ID を設定してください}")
        elif i > 1:
            cmd.append("--bootnodes=/ip4/127.0.0.1/tcp/30333/p2p/${CHAIN1_PEER_ID:?chain-1 の Local node identity を設定してください}")

        notes = []
        if is_validator:
            notes.append("# GRANDPA voter。keystore に gran 鍵の投入が別途必要 (README 参照)。")
        if is_miner:
            notes.append("# 採掘はこの 1 台だけ。複数採掘は reorg を招く。")
        deps = "      onion-proxy:\n        condition: service_started\n" if (a.bootnode_core and i == 1) else ""
        W(f"  chain-{i}:")
        W(f"    image: ${{ANARCHY_NODE_IMAGE:-{REGISTRY}/anarchy-node:latest}}")
        W("    restart: unless-stopped")
        W('    network_mode: "service:netns"')
        W("    depends_on:")
        W("      tor:")
        W("        condition: service_healthy")
        if deps: W(deps.rstrip("\n"))
        for n in notes: W("    " + n)
        W('    entrypoint: ["torsocks", "/usr/local/bin/anarchy-node"]')
        W("    command:")
        for c in cmd: W(f"      - {c}")
        W("    volumes:")
        W(f"      - chain{i}-data:/data")
        W("      - ./torsocks.conf:/etc/tor/torsocks.conf:ro")
        W("      - ./chainspec.json:/etc/anarchy/chainspec.json:ro")
        W("")

    for i in range(1, a.storage + 1):
        port = STORAGE_PORT_BASE + i
        ip = ip_storage(i)
        W(f"  storage-{i}:")
        W(f"    image: ${{ANARCHY_STORAGE_IMAGE:-{REGISTRY}/anarchy-storage-node:latest}}")
        W("    restart: unless-stopped")
        if remote_chain:
            W('    network_mode: "service:tor"')
            W("    depends_on:")
            W("      tor:")
            W("        condition: service_healthy")
            W("    # チェーンが別ホストなので torsocks で包む (--chain-url が .onion)")
            W('    entrypoint: ["torsocks", "/usr/local/bin/anarchy-storage-node"]')
        else:
            W("    depends_on:")
            W("      chain-1:")
            W("        condition: service_started")
            W("    # 同一ホストのチェーンに直結するので torsocks で包まない")
            W('    entrypoint: ["/usr/local/bin/anarchy-storage-node"]')
        W("    command:")
        W("      - --config=/etc/anarchy/storage.toml")
        W("      - --data-dir=/data")
        W(f"      - --rpc-port={port}")
        W(f"      - --public-url=${{STORAGE_PUBLIC_URL_BASE:?tor の anarchy-storage/hostname から設定してください}}:{port}")
        if remote_chain:
            W("      - --chain-url=ws://${CORE_CHAIN_ONION:?core の chain onion を設定してください}:9944")
        else:
            W(f"      - --chain-url=ws://{IP_NETNS}:9944")
        if not remote_chain:
            W("    networks:")
            W("      anarchy:")
            W(f"        ipv4_address: {ip}")
        W("    volumes:")
        W(f"      - storage{i}-data:/data")
        if remote_chain:
            W("      - ./torsocks.conf:/etc/tor/torsocks.conf:ro")
        W(f"      - ./storage-{i}.toml:/etc/anarchy/storage.toml:ro")
        W("")

    if has_netns or not remote_chain:
        W("networks:")
        W("  anarchy:")
        W("    ipam:")
        W("      config:")
        W(f"        - subnet: {SUBNET}")
        W("")
    W("volumes:")
    W("  tor-data:")
    for i in range(1, a.chains + 1): W(f"  chain{i}-data:")
    for i in range(1, a.storage + 1): W(f"  storage{i}-data:")
    (out / "compose.yml").write_text("\n".join(L) + "\n")

    # ------------------------------------------------------------------ torrc
    T = []
    T.append(f"# {a.out} 用 torrc (gen.py が生成)")
    T.append("#")
    T.append("# HiddenServicePort の転送先にホスト名は使えない (IP かソケットのみ)。")
    T.append("")
    T.append("DataDirectory /var/lib/tor")
    T.append("RunAsDaemon 0")
    T.append("Log notice stdout")
    T.append("")
    T.append("SocksPort 127.0.0.1:9050 IsolateClientAddr IsolateSOCKSAuth")
    if a.chains and not a.bootnode_core:
        # core: 他ホストから dial されるので p2p を公開。RPC は運用確認用
        T.append("")
        T.append("# ---- Hidden Service: チェーン ----")
        T.append("HiddenServiceDir /var/lib/tor/anarchy-chain")
        T.append("HiddenServiceVersion 3")
        for i in range(1, a.chains + 1):
            T.append(f"HiddenServicePort {P2P_BASE+i} 127.0.0.1:{P2P_BASE+i}")
        for i in range(1, a.chains + 1):
            T.append(f"HiddenServicePort {RPC_BASE+i}  127.0.0.1:{RPC_BASE+i}")
    if a.storage:
        T.append("")
        T.append(f"# ---- Hidden Service: ストレージ {a.storage} 台 ----")
        T.append("HiddenServiceDir /var/lib/tor/anarchy-storage")
        T.append("HiddenServiceVersion 3")
        for i in range(1, a.storage + 1):
            port = STORAGE_PORT_BASE + i
            target = "127.0.0.1" if remote_chain else ip_storage(i)
            T.append(f"HiddenServicePort {port} {target}:{port}")
    T += ["", "ClientUseIPv6 1", "ClientPreferIPv6ORPort 0", "SafeLogging 1", ""]
    (out / "torrc").write_text("\n".join(T))

    # ------------------------------------------------------------------ torsocks.conf
    (out / "torsocks.conf").write_text(textwrap.dedent("""\
        # gen.py が生成。TorAddress は IP のみ (ホスト名は解決されない)。
        TorAddress 127.0.0.1
        TorPort 9050
        AllowInbound 1
        # 同一ホストのノード同士を Tor に流さないために必須。
        # torsocks の除外設定はこれ (127.0.0.0/8) しか無い。
        AllowOutboundLocalhost 1
    """))

    # ------------------------------------------------------------------ storage.toml.tmpl
    if a.storage:
        cap = parse_size(a.capacity)
        (out / "storage.toml.tmpl").write_text(textwrap.dedent(f"""\
            # gen.py が生成。storage-N.toml に展開して signer_seed をノードごとに変えること:
            #   for i in $(seq 1 {a.storage}); do sed "s/CHANGE_ME/$(openssl rand -hex 32)/" storage.toml.tmpl > storage-$i.toml; done
            #
            # capacity は「宣言値」に対して超過チェックされる。実ディスクより大きく宣言すると
            # 「まだ空きがある」と思って書き込み続けディスク full で落ちるので、
            # ホストのディスクから逆算すること。
            capacity = {cap}          # {a.capacity} × {a.storage} 台

            declare_rate_limit = 10
            auth_enabled = true
            dev_mode = false
            listen_addr = "/ip4/0.0.0.0/tcp/4001"
            signer_seed = "CHANGE_ME"
        """))

    # ------------------------------------------------------------------ .env.example
    E = [f"# {a.out} の .env テンプレート (gen.py が生成)。cp .env.example .env して埋めること。", ""]
    if a.mine:
        E += ["# 採掘報酬の受取先 (自分の SS58)", "ANARCHY_COINBASE=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY", ""]
    if a.chains > 1:
        E += ["# chain-1 起動後: docker compose logs chain-1 | grep 'Local node identity'", "CHAIN1_PEER_ID=CHANGE_ME", ""]
    if a.bootnode_core or remote_chain:
        E += ["# core ホストの chain onion", "CORE_CHAIN_ONION=CHANGE_ME.onion", ""]
    if a.bootnode_core:
        E += ["# core の chain-1 peer ID", "CORE_CHAIN_PEER_ID=CHANGE_ME", ""]
    if a.storage:
        E += ["# tor 起動後: docker compose exec tor cat /var/lib/tor/anarchy-storage/hostname",
              "# ポート番号は付けない (compose が付ける)", "STORAGE_PUBLIC_URL_BASE=http://CHANGE_ME.onion", ""]
    if a.public_rpc:
        E += ["# フロントのオリジン (別オリジンなので CORS が要る)", "# ANARCHY_RPC_CORS=https://example.org", ""]
    E += ["# イメージ (省略時は ghcr の latest)"]
    if not remote_chain:
        E += [f"# ANARCHY_NODE_IMAGE={REGISTRY}/anarchy-node:latest"]
    E += [f"# ANARCHY_STORAGE_IMAGE={REGISTRY}/anarchy-storage-node:latest", ""]
    (out / ".env.example").write_text("\n".join(E))

    (out / ".gitignore").write_text("storage-*.toml\n.env\nchainspec.json\n")

    # ------------------------------------------------------------------ nginx (public-rpc)
    if a.public_rpc:
        rpc_port = RPC_BASE + 1
        (out / "nginx-anarchy.conf").write_text(textwrap.dedent(f"""\
            # {a.out} — 公開 wss エンドポイント (gen.py が生成)
            #
            # ⚠️ Cloudflare のプロキシ (オレンジ雲) は通さないこと。CF はアイドル WebSocket を
            #    閉じ、PAPI は ping を送らないためブロック間隔の隙間で切断される。DNS only にする。

            server {{
                listen 80;
                server_name {a.domain};
                location /.well-known/acme-challenge/ {{ root /var/www/html; }}
                location / {{ return 301 https://$host$request_uri; }}
            }}

            server {{
                # `http2 on;` は nginx 1.25.1+ の書式。Ubuntu 24.04 は 1.24 なので listen 行に付ける
                listen 443 ssl http2;
                server_name {a.domain};

                ssl_certificate     /etc/letsencrypt/live/{a.domain}/fullchain.pem;
                ssl_certificate_key /etc/letsencrypt/live/{a.domain}/privkey.pem;
                ssl_protocols TLSv1.2 TLSv1.3;

                location /rpc {{
                    # チェーンは netns の名前空間内で動くため、ホストの 127.0.0.1 からは見えない。
                    # compose で固定した netns の IP を指す (サブネットは gen.py の --subnet と一致)。
                    proxy_pass http://{IP_NETNS}:{rpc_port};
                    proxy_http_version 1.1;
                    proxy_set_header Upgrade $http_upgrade;
                    proxy_set_header Connection "upgrade";

                    # --rpc-cors を絞ると Substrate は Host ヘッダのフィルタも有効にし、
                    # listen アドレスの loopback 表現しか通さない (実測)。固定値にする。
                    # ブラウザの CORS 判定は Origin で行われるので制限は効いたまま。
                    proxy_set_header Host "localhost:{rpc_port}";

                    # デフォルト 60s では PoW のブロック間隔で WS が切れる
                    proxy_read_timeout  3600s;
                    proxy_send_timeout  3600s;
                }}
                location / {{ return 404; }}
            }}
        """))

    print(f"生成: {out}/")
    for f in ("compose.yml", "torrc", "torsocks.conf", "storage.toml.tmpl", ".env.example", ".gitignore"):
        if (out / f).exists(): print(f"  {f}")

if __name__ == "__main__":
    main()
