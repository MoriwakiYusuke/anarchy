FROM alpine:3.20

# tor package が tor ユーザ/グループを作成する
RUN apk add --no-cache tor \
    && mkdir -p /var/lib/tor \
    && chown -R tor:tor /var/lib/tor \
    && chmod 700 /var/lib/tor

USER tor
EXPOSE 9050 9051
CMD ["tor", "-f", "/etc/tor/torrc"]
