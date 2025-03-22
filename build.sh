#!/bin/bash

# build frontend
cd alu-panel
pnpm install
pnpm build
cd ..

# build backend
cargo build --release
