# Holds every crate crates.io would publish to what its page shows, then packages and builds it.
[metadata("rust")]
check-cargo-publish:
    mise exec -- python3 .just/cargo-publish.py metadata --book https://atomix-labs.github.io/atomix/
    mise exec -- python3 .just/cargo-publish.py check

# Publishes every crate crates.io does not have at its version yet, dependencies first.
publish-cargo-publish:
    mise exec -- python3 .just/cargo-publish.py publish
