# Build Stage
FROM ekidd/rust-musl-builder:latest AS build

ADD --chown=rust:rust . ./

RUN cargo build --package noodle-webapp --release

# Final Stage
FROM alpine:latest

RUN addgroup -g 1000 noodle
RUN adduser -D -s /bin/sh -u 1000 -G noodle noodle

WORKDIR /home/noodle/

RUN mkdir static wordlists
COPY --from=build --chown=noodle:noodle /home/rust/src/target/x86_64-unknown-linux-musl/release/noodle-webapp .
COPY --from=build /home/rust/src/noodle-webapp/static/* ./static/
COPY --from=build /home/rust/src/wordlists/* ./wordlists/

USER noodle
CMD ./noodle-webapp ./wordlists
