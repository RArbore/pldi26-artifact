FROM debian:bookworm-slim

RUN apt-get update
RUN apt-get -y install build-essential
RUN apt-get -y install curl
RUN apt-get -y install libgmp-dev
RUN apt-get -y install libmpc-dev
RUN apt-get -y install libmpfr-dev
RUN apt-get -y install wget

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path

ENV PATH="/root/.local/bin:/root/.cargo/bin:${PATH}"

WORKDIR /root

COPY Cargo.toml .
COPY Cargo.lock .
COPY opt-eqsat opt-eqsat
RUN cargo build --release

COPY paper_example1.c paper_example1.c
COPY paper_example2.c paper_example2.c

RUN wget "https://github.com/llvm/llvm-project/releases/download/llvmorg-21.1.0/LLVM-21.1.0-Linux-X64.tar.xz" && \
    tar -xf LLVM-21.1.0-Linux-X64.tar.xz && \
    ln LLVM-21.1.0-Linux-X64/bin/clang-21 clang && \
    rm -rf LLVM-21.1.0-Linux-X64 LLVM-21.1.0-Linux-X64.tar.xz 

RUN wget "https://mirrors.ocf.berkeley.edu/gnu/gcc/gcc-15.2.0/gcc-15.2.0.tar.gz" && \
    tar -xf gcc-15.2.0.tar.gz && \
    mkdir gcc-build && \
    cd gcc-build && ../gcc-15.2.0/configure --disable-bootstrap --enable-languages=c --disable-multilib && \
    make -j$(nproc) && make install && cd .. && \
    rm -rf gcc-build gcc-15.2.0 gcc-15.2.0.tar.gz

RUN ./clang -v
RUN gcc -v
