
node-install:
	bun install --save-dev mocha typescript ts-node
	bun install --save-dev @types/node @types/mocha

wasm-install:
	rustup target add wasm32-unknown-unknown
	cargo binstall wasm-pack

wasm-build:
	wasm-pack build --target web

nodejs-build:
	wasm-pack build --target nodejs

wasm-test:
	wasm-pack test --headless --firefox

build:
	cargo build

python-build:
	maturin develop --features lang-python --release

pytest:
	python -m unittest discover -s tests

