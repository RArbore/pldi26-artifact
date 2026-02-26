#!/bin/sh

set -e

mkdir -p artifact

podman build -t opt-eqsat:latest .
podman save opt-eqsat:latest | pigz > artifact/opt-eqsat.tar.gz

echo "set -e" > artifact/run.sh
echo "podman run opt-eqsat:latest cargo test --release paper_example" >> artifact/run.sh
echo "podman run opt-eqsat:latest sh -c \"./clang -O2 -S paper_example1.c -o paper_example1_clang.S && cat paper_example1_clang.S\" > paper_example1_clang.S" >> artifact/run.sh
echo "podman run opt-eqsat:latest sh -c \"./clang -O2 -S paper_example2.c -o paper_example2_clang.S && cat paper_example2_clang.S\" > paper_example2_clang.S" >> artifact/run.sh
echo "podman run opt-eqsat:latest sh -c \"gcc -O2 -S paper_example1.c -o paper_example1_gcc.S && cat paper_example1_gcc.S\" > paper_example1_gcc.S" >> artifact/run.sh
echo "podman run opt-eqsat:latest sh -c \"gcc -O2 -S paper_example2.c -o paper_example2_gcc.S && cat paper_example2_gcc.S\" > paper_example2_gcc.S" >> artifact/run.sh
echo "echo \"All done!\"" >> artifact/run.sh
chmod +x artifact/run.sh

cp expected_paper_example1_clang.S artifact/
cp expected_paper_example2_clang.S artifact/
cp expected_paper_example1_gcc.S artifact/
cp expected_paper_example2_gcc.S artifact/

zip -r artifact/code.zip Cargo.lock Cargo.toml rustfmt.toml opt-eqsat README.md ARCHITECTURE.md

cp README.md artifact/
