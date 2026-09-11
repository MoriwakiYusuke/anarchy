#!/usr/bin/env bash
# AWS に storage-only ホスト (t3.micro) を 1 台作る。冪等 (既にあれば作らない)。
#
#   AWS_PROFILE=anarchy ./infra/deploy/aws-storage-only.sh          # 作成 + IP 表示
#   AWS_PROFILE=anarchy ./infra/deploy/aws-storage-only.sh stop     # 停止 (課金はEBSのみ)
#   AWS_PROFILE=anarchy ./infra/deploy/aws-storage-only.sh start
#   AWS_PROFILE=anarchy ./infra/deploy/aws-storage-only.sh destroy  # インスタンス削除 (SG / key は残す)
#
# 作るもの:
#   - key pair  anarchy            ← ~/.ssh/id_ed25519.pub を import
#   - SG        anarchy-storage-only  inbound は 22/tcp のみ。ストレージは Tor HS 経由でしか出さない
#   - instance  anarchy-storage-only  t3.micro / Ubuntu 24.04 / gp3 30GB / CPU credits = standard
#
# CPU credits を standard にする理由: unlimited (t3 の既定) だとバーストが課金される。
# storage-node は 10 台でも CPU をほぼ使わないので standard で足りる。
#
# ルートボリューム 30GB は無料枠の上限。storage×10 × 2GiB = 20GiB を宣言している根拠
# (gen.py storage-only --capacity 2G)。
set -euo pipefail

REGION=${AWS_REGION:-ap-northeast-1}
NAME=anarchy-storage-only
KEY_NAME=anarchy
PUBKEY=${PUBKEY:-$HOME/.ssh/id_ed25519.pub}
aws() { command aws --region "$REGION" --output text "$@"; }
log() { printf '\n\033[36m==> %s\033[0m\n' "$*"; }

instance_id() {
  aws ec2 describe-instances \
    --filters "Name=tag:Name,Values=$NAME" "Name=instance-state-name,Values=pending,running,stopping,stopped" \
    --query 'Reservations[].Instances[].InstanceId'
}
public_ip() {
  aws ec2 describe-instances --instance-ids "$1" --query 'Reservations[].Instances[].PublicIpAddress'
}

case "${1:-create}" in
  stop)    aws ec2 stop-instances  --instance-ids "$(instance_id)" >/dev/null; log "停止中"; exit ;;
  start)   ID=$(instance_id); aws ec2 start-instances --instance-ids "$ID" >/dev/null
           aws ec2 wait instance-running --instance-ids "$ID"
           log "起動: ubuntu@$(public_ip "$ID")  (IP は停止/起動で変わる)"; exit ;;
  destroy) ID=$(instance_id); [ -n "$ID" ] && aws ec2 terminate-instances --instance-ids "$ID" >/dev/null
           log "削除: $ID"; exit ;;
  create)  ;;
  *) echo "usage: $0 [create|stop|start|destroy]" >&2; exit 2 ;;
esac

log "認証確認"
aws sts get-caller-identity --query 'Arn'

log "key pair: $KEY_NAME"
if ! aws ec2 describe-key-pairs --key-names "$KEY_NAME" >/dev/null 2>&1; then
  aws ec2 import-key-pair --key-name "$KEY_NAME" --public-key-material "fileb://$PUBKEY" >/dev/null
  echo "imported $PUBKEY"
else
  echo "exists"
fi

log "security group: $NAME (22/tcp のみ)"
VPC=$(aws ec2 describe-vpcs --filters Name=is-default,Values=true --query 'Vpcs[0].VpcId')
SG=$(aws ec2 describe-security-groups --filters "Name=group-name,Values=$NAME" "Name=vpc-id,Values=$VPC" \
       --query 'SecurityGroups[0].GroupId' 2>/dev/null || true)
if [ -z "$SG" ] || [ "$SG" = "None" ]; then
  SG=$(aws ec2 create-security-group --group-name "$NAME" --vpc-id "$VPC" \
         --description "anarchy storage-only: ssh only, storage is Tor HS" --query GroupId)
  aws ec2 authorize-security-group-ingress --group-id "$SG" --protocol tcp --port 22 --cidr 0.0.0.0/0 >/dev/null
  echo "created $SG"
else
  echo "exists $SG"
fi

log "instance: $NAME"
ID=$(instance_id)
if [ -z "$ID" ]; then
  AMI=$(aws ssm get-parameter \
          --name /aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id \
          --query 'Parameter.Value')
  echo "AMI $AMI (Ubuntu 24.04)"
  ID=$(aws ec2 run-instances \
         --image-id "$AMI" --instance-type t3.micro --key-name "$KEY_NAME" \
         --security-group-ids "$SG" \
         --credit-specification CpuCredits=standard \
         --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=30,VolumeType=gp3,DeleteOnTermination=true}' \
         --metadata-options HttpTokens=required \
         --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$NAME},{Key=project,Value=anarchy}]" \
                              "ResourceType=volume,Tags=[{Key=Name,Value=$NAME},{Key=project,Value=anarchy}]" \
         --query 'Instances[0].InstanceId')
  echo "created $ID"
else
  echo "exists $ID"
fi
aws ec2 wait instance-running --instance-ids "$ID"
IP=$(public_ip "$ID")

cat <<NEXT

ubuntu@$IP

次:
  ssh ubuntu@$IP 'curl -fsSL https://raw.githubusercontent.com/MoriwakiYusuke/anarchy/main/infra/deploy/bootstrap.sh | bash'
  ssh ubuntu@$IP   # 再ログイン (docker グループ反映)
  git clone https://github.com/MoriwakiYusuke/anarchy.git && cd anarchy/infra/deploy/storage-only
  # 以降 infra/deploy/README.md「生成後の手順」(chainspec は不要、CORE_CHAIN_ONION を core から)
NEXT
