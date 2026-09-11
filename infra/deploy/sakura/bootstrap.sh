#!/usr/bin/env bash
# さくら VPS の初期セットアップ。初回ログイン後に 1 回だけ実行する。
#
#   curl -fsSL https://raw.githubusercontent.com/MoriwakiYusuke/anarchy/main/infra/deploy/sakura/bootstrap.sh | bash
# または clone 後に
#   ./infra/deploy/sakura/bootstrap.sh
#
# やること:
#   - パッケージ更新
#   - タイムゾーンを UTC に (GCP は US リージョンなのでログを突き合わせやすくする)
#   - swap 2GB + swappiness=10
#   - Docker
#
# やらないこと:
#   - ufw は入れない。さくら外部のパケットフィルタ (22番のみ開放) と重複するため
#   - ポートも開けない。チェーン/ストレージは Tor の Hidden Service 経由でのみ公開する
set -euo pipefail

log() { printf '\n\033[36m==> %s\033[0m\n' "$*"; }

log "パッケージ更新"
sudo apt-get update -qq
sudo DEBIAN_FRONTEND=noninteractive apt-get upgrade -y -qq

log "タイムゾーンを UTC に"
sudo timedatectl set-timezone UTC

log "swap 2GB"
if swapon --show | grep -q '/swapfile'; then
    echo "  既に有効: $(swapon --show --noheadings --bytes | awk '{printf "%.1fGB\n", $3/1024/1024/1024}')"
else
    sudo fallocate -l 2G /swapfile
    sudo chmod 600 /swapfile
    sudo mkswap /swapfile > /dev/null
    sudo swapon /swapfile
    grep -q '^/swapfile' /etc/fstab || echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab > /dev/null
fi

# 常用させない。デフォルトの 60 だと RAM に余裕があっても swap に追い出すため、
# RocksDB / RandomX のレイテンシが落ちる。保険としてだけ使わせる。
log "vm.swappiness=10"
echo 'vm.swappiness=10' | sudo tee /etc/sysctl.d/99-swappiness.conf > /dev/null
sudo sysctl -q -p /etc/sysctl.d/99-swappiness.conf

log "Docker"
if command -v docker > /dev/null 2>&1; then
    echo "  既にインストール済み: $(docker --version)"
else
    curl -fsSL https://get.docker.com | sudo sh
    sudo usermod -aG docker "$USER"
    echo "  ※ docker グループを反映するため、一度ログアウト/ログインしてください"
fi

log "確認"
echo "--- メモリと swap ---"; free -h
echo "--- swappiness ---"; cat /proc/sys/vm/swappiness
echo "--- タイムゾーン ---"; timedatectl | grep "Time zone"
echo "--- Docker ---"; sudo docker version --format '{{.Server.Version}}' 2>/dev/null || echo "(未起動)"

cat <<'NEXT'

次の手順:
  git clone https://github.com/MoriwakiYusuke/anarchy.git
  cd anarchy/infra/deploy/sakura
  cp ../anarchy-portfolio-raw.json chainspec.json
  for i in 1 2 3; do sed "s/CHANGE_ME/$(openssl rand -hex 32)/" storage.toml.tmpl > storage-$i.toml; done
  cp .env.example .env      # ANARCHY_COINBASE を自分の SS58 に
  docker compose up -d netns tor
  docker compose exec tor cat /var/lib/tor/anarchy-chain/hostname     # 控える
  docker compose exec tor cat /var/lib/tor/anarchy-storage/hostname   # .env に設定
  docker compose up -d chain-1
  docker compose logs chain-1 | grep "Local node identity"            # .env の CHAIN1_PEER_ID に
  docker compose up -d

詳細は docs/operations/deployment-multi-provider.md
NEXT
