#!/bin/sh

set -e

mkdir -p artifact

podman build -t opt-eqsat:latest .
podman save opt-eqsat:latest | pigz > artifact/opt-eqsat.tar.gz

echo "set -e" > artifact/test.sh
echo "podman run opt-eqsat:latest cargo test --release paper_example" >> artifact/test.sh
echo "podman run opt-eqsat:latest sh -c \"./clang -O2 -S paper_example1.c -o paper_example1_clang.S && cat paper_example1_clang.S\" > paper_example1_clang.S" >> artifact/test.sh
echo "podman run opt-eqsat:latest sh -c \"./clang -O2 -S paper_example2.c -o paper_example2_clang.S && cat paper_example2_clang.S\" > paper_example2_clang.S" >> artifact/test.sh
echo "podman run opt-eqsat:latest sh -c \"gcc -O2 -S paper_example1.c -o paper_example1_gcc.S && cat paper_example1_gcc.S\" > paper_example1_gcc.S" >> artifact/test.sh
echo "podman run opt-eqsat:latest sh -c \"gcc -O2 -S paper_example2.c -o paper_example2_gcc.S && cat paper_example2_gcc.S\" > paper_example2_gcc.S" >> artifact/test.sh
echo "echo \"All done!\"" >> artifact/test.sh
chmod +x artifact/test.sh

echo "set -e" > artifact/eval.sh
echo "podman run opt-eqsat:latest cargo run --release" >> artifact/eval.sh
echo "echo \"All done!\"" >> artifact/eval.sh
chmod +x artifact/eval.sh

cp expected_paper_example1_clang.S artifact/
cp expected_paper_example2_clang.S artifact/
cp expected_paper_example1_gcc.S artifact/
cp expected_paper_example2_gcc.S artifact/
cp README.md artifact/
cp table.png artifact/

zip -r artifact/code.zip Cargo.lock Cargo.toml rustfmt.toml opt-eqsat README.md ARCHITECTURE.md
