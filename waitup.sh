until curl -s --max-time 4 http://127.0.0.1:9333/json/version 2>/dev/null | grep -q Browser; do sleep 3; done
echo "epoch up"
