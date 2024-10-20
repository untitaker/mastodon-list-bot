FROM node:21-alpine3.20 AS frontend-builder

WORKDIR /app
COPY package-lock.json package.json buildscript.js ./
COPY src ./src/
RUN npm ci
RUN npm run build

FROM rust:1.82-alpine3.20 AS builder

RUN mkdir -p ~/.cargo && \
    echo '[registries.crates-io]' > ~/.cargo/config && \
    echo 'protocol = "sparse"' >> ~/.cargo/config

RUN apk add --no-cache libc-dev

RUN USER=root cargo new --bin /app
WORKDIR /app

# Just copy the Cargo.toml files and trigger
# a build so that we compile our dependencies only.
# This way we avoid layer cache invalidation
# if our dependencies haven't changed,
# resulting in faster builds.

COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo build --release && rm -rf src/

# Copy the source code and run the build again.
# This should only compile the app itself as the
# dependencies were already built above.
COPY . ./
COPY --from=frontend-builder /app/build/ /app/build/
COPY --from=frontend-builder /app/node_modules/ /app/node_modules/
RUN rm ./target/release/deps/mastodon_list_bot* && cargo build --release
RUN strip target/release/mastodon-list-bot

# Our production image starts here, which uses
# the files from the builder image above.
FROM alpine:3.18

COPY --from=builder /app/target/release/mastodon-list-bot /usr/local/bin/mastodon-list-bot

RUN apk add --no-cache tini

RUN addgroup -S app && adduser -S app -G app
USER app

ENTRYPOINT ["/sbin/tini", "--", "mastodon-list-bot"]
EXPOSE 3000
