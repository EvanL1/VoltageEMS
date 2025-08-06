FROM redis:8-alpine

# Copy Lua functions
COPY scripts/redis-functions/*.lua /usr/local/redis/functions/

# Copy initialization script
COPY docker/redis/init-functions.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/init-functions.sh

# Custom entrypoint to load functions on startup
COPY docker/redis/docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["redis-server"]