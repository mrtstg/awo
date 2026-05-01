run:
	cargo run

install:
	cargo install --path .

build-static-glibc:
	RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-gnu

build-static-musl:
	cargo build --release --target x86_64-unknown-linux-musl
