#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "=== Phase 1 Autotest: OTP Auth ==="
echo "[1/5] Ensuring postgres+redis healthy..."
docker compose up -d postgres redis
for i in {1..30}; do
  if docker compose exec -T postgres pg_isready -U messenger_user -d messenger >/dev/null 2>&1; then break; fi
  echo "  waiting postgres $i/30"
  sleep 1
done
docker compose exec -T postgres pg_isready -U messenger_user -d messenger

echo "[2/5] Running migrations..."
# Uses DATABASE_URL from .env (expand not needed as we set concrete in script)
export DATABASE_URL="postgres://messenger_user:1234@localhost:5432/messenger"
# Try host-mapped port, fallback to docker network
cargo sqlx migrate run --manifest-path server/Cargo.toml 2>&1 | tail -n 20 || echo "migrate via cargo sqlx may need local sqlx-cli; trying via psql"
# Ensure phone_verification table exists (idempotent)
docker compose exec -T postgres psql -U messenger_user -d messenger -c "\d phone_verification" >/dev/null 2>&1 || {
  echo "Applying 001_init.sql directly"
  docker compose exec -T postgres psql -U messenger_user -d messenger -f /dev/stdin < server/migrations/001_init.sql || true
}

echo "[3/5] cargo test (unit + integration) -- --test-threads=1"
cargo test --manifest-path server/Cargo.toml -- --nocapture --test-threads=1

echo "[4/5] grpcurl E2E (requires server running on 50051)"
if curl -s http://localhost:50051 >/dev/null 2>&1 || nc -z localhost 50051 2>/dev/null; then
  echo "  server reachable, running grpcurl..."
  # Start server if not running (in background)
  if ! docker compose ps server | grep -q "Up"; then
    echo "  starting server..."
    docker compose up -d server --build
    sleep 5
  fi
  grpcurl -plaintext localhost:50051 list 2>&1 | grep -q "messenger.AuthService" && echo "  AuthService present"
  PHONE="+79990001122"
  echo "  RequestOTP $PHONE"
  RESP=$(grpcurl -plaintext -d "{\"phone\":\"$PHONE\"}" localhost:50051 messenger.AuthService/RequestOTP 2>&1) || true
  echo "$RESP"
  OTP=$(echo "$RESP" | grep -o '"debugOtp": *"[^"]*"' | cut -d'"' -f4 | tr -d ' ')
  if [ -z "$OTP" ]; then
    OTP=$(echo "$RESP" | grep -o '[0-9]\{6\}' | head -n1)
  fi
  if [ -n "$OTP" ]; then
    echo "  VerifyOTP $OTP"
    grpcurl -plaintext -d "{\"phone\":\"$PHONE\",\"code\":\"$OTP\",\"username\":\"autotest\"}" localhost:50051 messenger.AuthService/VerifyOTP 2>&1 | head -n 20
  else
    echo "  Could not extract OTP (SMS_MOCK may be false, check logs: docker compose logs server | grep SMS_MOCK)"
  fi
else
  echo "  server not reachable on 50051, skipping grpcurl E2E (run: docker compose up -d --build && curl)"
fi

echo "[5/5] psql cleanup check"
docker compose exec -T postgres psql -U messenger_user -d messenger -c "SELECT phone, attempts, expires_at FROM phone_verification ORDER BY created_at DESC LIMIT 5;" || true
echo "  COUNT after should be 0 for verified phones:"
docker compose exec -T postgres psql -U messenger_user -d messenger -c "SELECT COUNT(*) FROM phone_verification;" || true

echo "=== Phase 1 Autotest DONE ==="
echo "If all cargo tests passed and grpcurl returned token, Phase 1 is verified."
