#!/usr/bin/env bash
# ホスト (core / gateway / storage-only 共通) の初期セットアップ。
# 初回ログイン後に 1 回だけ実行する。役割による差は無い (差は compose 側にある)。
#
#   curl -fsSL https://raw.githubusercontent.com/MoriwakiYusuke/anarchy/main/infra/deploy/bootstrap.sh | bash
# または clone 後に
#   ./infra/deploy/bootstrap.sh
#
# やること:
#   - パッケージ更新
#   - タイムゾーンを UTC に (ホストが別リージョンに散るのでログを突き合わせやすくする)
#   - swap 2GB + swappiness=10
#   - Docker
#
# やらないこと:
#   - ufw は入れない。事業者側のパケットフィルタ (22番のみ開放) と重複するため
#   - ポートも開けない。チェーン/ストレージは Tor の Hidden Service 経由でのみ公開する
set -euo pipefail

log() { printf '\n\033[36m==> %s\033[0m\n' "$*"; }

# Ubuntu は起動直後に unattended-upgrades が走り dpkg のロックを握る。
# そこに重ねると "Could not get lock /var/lib/dpkg/lock-frontend" で落ちるので待つ。
wait_for_apt_lock() {
    local waited=0
    while sudo fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1; do
        if [ "$waited" -ge 300 ]; then
            echo "  dpkg のロックが 5 分解放されません。以下を確認してください:" >&2
            ps -eo pid,etime,cmd | grep -E "apt|dpkg" | grep -v grep >&2
            return 1
        fi
        [ "$waited" = 0 ] && echo "  dpkg のロック待ち (unattended-upgrades が走っている可能性)..."
        sleep 10; waited=$((waited + 10))
    done
}

# デフォルトの archive.ubuntu.com / security.ubuntu.com に到達できない環境がある
# (さくらの VPS で実際に両方 15 秒タイムアウトした。ネットワーク自体は正常で
#  github:443 も example.com:80 も通るのにミラーだけ死んでいる)。
# 到達性を確認し、駄目なら国内ミラーに切り替える。
ensure_apt_mirror() {
    if curl -4 -s -o /dev/null -m 10 http://archive.ubuntu.com/ubuntu/; then
        return 0
    fi
    echo "  archive.ubuntu.com に到達できません。国内ミラー (JAIST) に切り替えます"
    local src=/etc/apt/sources.list.d/ubuntu.sources
    [ -f "$src" ] || src=/etc/apt/sources.list
    sudo cp "$src" "${src}.bak.$(date +%s)"
    sudo sed -i \
        -e "s|http://archive.ubuntu.com/ubuntu/|http://ftp.jaist.ac.jp/pub/Linux/ubuntu/|g" \
        -e "s|http://security.ubuntu.com/ubuntu/|http://ftp.jaist.ac.jp/pub/Linux/ubuntu/|g" \
        "$src"
}

log "パッケージ更新"
wait_for_apt_lock
ensure_apt_mirror
sudo apt-get update -qq
wait_for_apt_lock
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
    wait_for_apt_lock
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
  cd anarchy/infra/deploy/<core|gateway|storage-only>
  # 以降は infra/deploy/README.md の「生成後の手順」

詳細は docs/operations/deployment-multi-provider.md
NEXT
