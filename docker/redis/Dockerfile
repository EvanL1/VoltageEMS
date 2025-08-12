FROM redis:8-alpine

# Copy all Lua functions
COPY scripts/redis-functions/*.lua /data/

# Copy startup script
COPY docker/redis/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Health check
HEALTHCHECK --interval=10s --timeout=5s --retries=5 \
    CMD redis-cli ping || exit 1

# Use the entrypoint script to start Redis and load functions
ENTRYPOINT ["/entrypoint.sh"]