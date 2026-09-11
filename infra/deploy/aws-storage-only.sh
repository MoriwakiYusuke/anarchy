#!/usr/bin/env bash
# AWS Lightsail に storage-only ホストを 1 台作る。冪等 (既にあれば作らない)。
#
#   AWS_PROFILE=anarchy ./infra/deploy/aws-storage-only.sh          # 作成 + IP 表示
#   AWS_PROFILE=anarchy ./infra/deploy/aws-storage-only.sh stop     # 停止 (Lightsail は停止中も定額課金。節約にはならない)
#   AWS_PROFILE=anarchy ./infra/deploy/aws-storage-only.sh start
#   AWS_PROFILE=anarchy ./infra/deploy/aws-storage-only.sh destroy  # インスタンス + 静的 IP を削除
#
# EC2 ではなく Lightsail にした理由 (2026-09-12):
#   EC2 t3.micro は本体 $9.9 + パブリック IPv4 $3.65 + EBS 30GB $2.9 = 月 $16.5。
#   このアカウントは 2025-01 作成で 12 ヶ月無料枠が切れている。
#   Lightsail nano は IP・20GB SSD・1TB 転送込みで月 $5 固定。常時起動が前提なのでこちら。
#   IPv6-only の nano ($3.50) もあるが ghcr.io / github.com が IPv6 非対応で
#   docker pull を Tor 経由にする細工が要るため、dual-stack を選んだ。
#
# 作るもの:
#   - key pair   anarchy               ← ~/.ssh/id_ed25519.pub を import
#   - instance   anarchy-storage-only  nano_3_0 (512MB / 2vCPU / 20GB) Ubuntu 24.04
#   - static IP  anarchy-storage-only-ip (attach 中は無料。stop/start で IP が変わらないように)
#   - firewall   22/tcp のみ (Lightsail 既定の 80 は閉じる)。ストレージは Tor HS 経由でしか出さない
set -euo pipefail

REGION=${AWS_REGION:-ap-northeast-1}
ZONE=${AWS_ZONE:-${REGION}a}
NAME=anarchy-storage-only
KEY_NAME=anarchy
SIP_NAME=$NAME-ip          # Lightsail はリソース種別を跨いで名前が一意なのでインスタンス名と同じにできない
BUNDLE=${BUNDLE:-nano_3_0}
PUBKEY=${PUBKEY:-$HOME/.ssh/id_ed25519.pub}
aws() { command aws --region "$REGION" --output text "$@"; }
log() { printf '\n\033[36m==> %s\033[0m\n' "$*"; }

exists() { aws lightsail get-instance --instance-name "$NAME" --query 'instance.name' 2>/dev/null; }
state()  { aws lightsail get-instance-state --instance-name "$NAME" --query 'state.name'; }
ip()     { aws lightsail get-instance --instance-name "$NAME" --query 'instance.publicIpAddress'; }
wait_running() { until [ "$(state)" = running ]; do sleep 5; done; }

case "${1:-create}" in
  stop)    aws lightsail stop-instance  --instance-name "$NAME" >/dev/null; log "停止中 (課金は止まらない)"; exit ;;
  start)   aws lightsail start-instance --instance-name "$NAME" >/dev/null; wait_running; log "起動: ubuntu@$(ip)"; exit ;;
  destroy) aws lightsail delete-instance --instance-name "$NAME" >/dev/null 2>&1 && echo "instance 削除"
           aws lightsail release-static-ip --static-ip-name "$SIP_NAME" >/dev/null 2>&1 && echo "static ip 解放"
           log "削除完了 (key pair $KEY_NAME は残す)"; exit ;;
  create)  ;;
  *) echo "usage: $0 [create|stop|start|destroy]" >&2; exit 2 ;;
esac

log "認証確認"
aws sts get-caller-identity --query 'Arn'

log "key pair: $KEY_NAME"
if ! aws lightsail get-key-pair --key-pair-name "$KEY_NAME" >/dev/null 2>&1; then
  aws lightsail import-key-pair --key-pair-name "$KEY_NAME" \
    --public-key-base64 "$(cat "$PUBKEY")" >/dev/null   # CLI が base64 化する。自分で encode すると "format not valid"
  echo "imported $PUBKEY"
else
  echo "exists"
fi

log "instance: $NAME ($BUNDLE, $ZONE)"
if [ -z "$(exists)" ]; then
  aws lightsail create-instances --instance-names "$NAME" --availability-zone "$ZONE" \
    --blueprint-id ubuntu_24_04 --bundle-id "$BUNDLE" --key-pair-name "$KEY_NAME" \
    --tags key=project,value=anarchy >/dev/null
  echo "created"
else
  echo "exists"
fi
wait_running

log "static IP: $SIP_NAME"
if ! aws lightsail get-static-ip --static-ip-name "$SIP_NAME" >/dev/null 2>&1; then
  aws lightsail allocate-static-ip --static-ip-name "$SIP_NAME" >/dev/null
  echo "allocated"
fi
if [ "$(aws lightsail get-static-ip --static-ip-name "$SIP_NAME" --query 'staticIp.isAttached')" != "True" ]; then
  aws lightsail attach-static-ip --static-ip-name "$SIP_NAME" --instance-name "$NAME" >/dev/null
  echo "attached"
else
  echo "attached (already)"
fi

log "firewall: 22/tcp のみ"
aws lightsail put-instance-public-ports --instance-name "$NAME" \
  --port-infos fromPort=22,toPort=22,protocol=tcp >/dev/null
aws lightsail get-instance-port-states --instance-name "$NAME" \
  --query 'portStates[].[fromPort,protocol,state]' --output text

IP=$(ip)
cat <<NEXT

ubuntu@$IP

次:
  ssh ubuntu@$IP 'curl -fsSL https://raw.githubusercontent.com/MoriwakiYusuke/anarchy/main/infra/deploy/bootstrap.sh | bash'
  ssh ubuntu@$IP   # 再ログイン (docker グループ反映)
  git clone https://github.com/MoriwakiYusuke/anarchy.git && cd anarchy/infra/deploy/storage-only
  # 以降 infra/deploy/README.md「生成後の手順」(chainspec は不要、CORE_CHAIN_ONION を core から)
NEXT
