#!/bin/bash
set -e

echo "Running database migrations..."
sqlx migrate run

echo "Starting application..."
./target/release/crypto-kyo