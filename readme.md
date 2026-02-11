# 1. Start server
cargo run &

# 2. Login as admin
ADMIN_TOKEN=$(curl -s -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "admin@securechat.local",
    "password": "admin123"
  }' | jq -r '.access_token')

# 3. Create invitation
INVITE=$(curl -s -X POST http://localhost:3000/api/invitations \
  -H "Authorization: Bearer $ADMIN_TOKEN" | jq -r '.token')

echo "Invitation token: $INVITE"

# 4. Register new user
curl -X POST http://localhost:3000/api/auth/register \
  -H "Content-Type: application/json" \
  -d "{
    \"username\": \"testuser\",
    \"email\": \"testuser@example.com\",
    \"password\": \"password123\",
    \"invitation_token\": \"$INVITE\"
  }"

# 5. Login as new user
USER_TOKEN=$(curl -s -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "testuser@example.com",
    "password": "password123"
  }' | jq -r '.access_token')

# 6. Test WebSocket chat (in separate terminals)
# Terminal 1:
wscat -c "ws://localhost:3000/ws?token=$ADMIN_TOKEN"
# Send: {"content": "Hello from admin!"}

# Terminal 2:
wscat -c "ws://localhost:3000/ws?token=$USER_TOKEN"
# Send: {"content": "Hello from user!"}

# 7. Check message history
curl http://localhost:3000/api/messages \
  -H "Authorization: Bearer $ADMIN_TOKEN" | jq

# 8. Verify steganography in database
psql -U chatapp -d chatapp_db -h localhost -c "SELECT id, content FROM messages ORDER BY created_at DESC LIMIT 3;"
```

**Expected Results:**
- Messages appear in both WebSocket clients with username and timestamp
- Message history shows all messages in chronological order
- Database shows encoded (gibberish) content, not plaintext
- Messages include proper user attribution

### 5.4 Quick Reference - All Endpoints
```
Public Endpoints:
- POST /api/auth/register       - Register with invitation token
- POST /api/auth/login          - Login (returns access + refresh tokens)
- POST /api/auth/refresh        - Refresh access token

Protected Endpoints (require JWT):
- GET  /api/auth/me             - Get current user info
- POST /api/invitations         - Create invitation (admin only)
- GET  /api/messages            - Get message history
- GET  /ws?token=<JWT>          - WebSocket connection for real-time chat

Utility:
- GET  /health                  - Health check