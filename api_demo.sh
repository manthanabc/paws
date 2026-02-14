#!/bin/bash
# API Demo Script for Paws Server
# This demonstrates the task-based API for the web frontend

set -e

BASE_URL="${1:-http://localhost:3010}"
echo "=== Paws API Demo ==="
echo "Server: $BASE_URL"
echo ""

# 1. Health check
echo "1. Health Check"
echo "GET /api/health"
curl -s "$BASE_URL/api/health"
echo -e "\n"

# 2. Get environment info
echo "2. Get Environment"
echo "GET /api/env"
curl -s "$BASE_URL/api/env" | jq '.cwd'
echo ""

# 3. List available agents
echo "3. List Agents"
echo "GET /api/agents"
curl -s "$BASE_URL/api/agents" | jq '.[].id'
echo ""

# 4. Set active agent
echo "4. Set Active Agent"
echo "POST /api/config/active-agent"
curl -s -X POST "$BASE_URL/api/config/active-agent" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "paws"}'
echo -e "\n"

# 5. Create a conversation
echo "5. Create Conversation"
echo "POST /api/conversations"
CONV_ID=$(uuidgen)
curl -s -X POST "$BASE_URL/api/conversations" \
  -H "Content-Type: application/json" \
  -d "{\"id\": \"$CONV_ID\", \"title\": \"API Demo Conversation\"}"
echo -e "\n"
echo "Conversation ID: $CONV_ID"
echo ""

# 6. List conversations
echo "6. List Conversations"
echo "GET /api/conversations"
curl -s "$BASE_URL/api/conversations" | jq '.[0]'
echo ""

# 7. Create a task (submit a message for processing)
echo "7. Create Task"
echo "POST /api/tasks"
TASK_RESP=$(curl -s -X POST "$BASE_URL/api/tasks" \
  -H "Content-Type: application/json" \
  -d "{\"conversation_id\": \"$CONV_ID\", \"message\": \"What is 2+2? Just give me the number.\"}")
echo "$TASK_RESP" | jq '.'
TASK_ID=$(echo "$TASK_RESP" | jq -r '.task_id')
echo "Task ID: $TASK_ID"
echo ""

# 8. Get task status
echo "8. Get Task Status (polling...)"
echo "GET /api/tasks/:id"
for i in {1..10}; do
  STATUS=$(curl -s "$BASE_URL/api/tasks/$TASK_ID")
  STATUS_TYPE=$(echo "$STATUS" | jq -r '.status.type')
  echo "  Status: $STATUS_TYPE"
  if [ "$STATUS_TYPE" = "completed" ] || [ "$STATUS_TYPE" = "failed" ]; then
    break
  fi
  sleep 2
done
echo "$STATUS" | jq '.'
echo ""

# 9. Get task events (for reconnection)
echo "9. Get Task Events"
echo "GET /api/tasks/:id/events"
curl -s "$BASE_URL/api/tasks/$TASK_ID/events" | jq '.'
echo ""

# 10. List all tasks
echo "10. List All Tasks"
echo "GET /api/tasks"
curl -s "$BASE_URL/api/tasks" | jq '.[].status.type'
echo ""

# 11. Get conversation messages
echo "11. Get Conversation Messages"
echo "GET /api/conversations/:id"
curl -s "$BASE_URL/api/conversations/$CONV_ID" | jq '.messages[-1]'
echo ""

# 12. Test SSE stream (for 5 seconds)
echo "12. SSE Stream Test (5 seconds)"
echo "GET /api/tasks/:id/stream"
timeout 5 curl -s -N "$BASE_URL/api/tasks/$TASK_ID/stream" 2>/dev/null | head -5 || echo "(stream ended or no new events)"
echo ""

# 13. Get available models
echo "13. List Models"
echo "GET /api/models"
curl -s "$BASE_URL/api/models" | jq '.[0]'
echo ""

# 14. Get available skills
echo "14. List Skills"
echo "GET /api/skills"
curl -s "$BASE_URL/api/skills" | jq '.[].name'
echo ""

# 15. Cleanup - Delete conversation
echo "15. Delete Conversation"
echo "DELETE /api/conversations/:id"
curl -s -X DELETE "$BASE_URL/api/conversations/$CONV_ID"
echo -e "\n"

echo "=== Demo Complete ==="
