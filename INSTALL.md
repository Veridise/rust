# Example Installation Guide

This lists out several steps that you can follow to build this customized version of Rust (1.68.2 with LLVM 13). It's tested under AWS machines, so you may adjust it to fit your target machine accordingly.

## Steps

The following shows all the steps in a script. Check out comments for different sections, and you can skip them if the requirements are already satisfied.

```bash
# prepare the necessary utils for building
sudo apt-get update
sudo apt-get install build-essential zlib1g-dev ninja-build cmake clang binutils libssl-dev pkg-config

# pre-install llvm quickly
# note: this section can be simplified if you already have a working version ready
#       then simply modify the related LLVM paths in the following sections
wget https://apt.llvm.org/llvm.sh
chmod +x llvm.sh
sudo ./llvm.sh 13

# clone this rust repository and check out the correct branch
# note: replace the address to the git repo with whichever you have access to
git clone https://github.com/Veridise/rust.git
cd rust/
git checkout 1.68.2.llvm13

# if you need lld, then you need llvm-project pulled
git submodule update --init --recursive

# fix lld compilation file not found errors
sudo ln -s /usr/bin/llvm-config-13 /usr/bin/llvm-config
sudo ln -s /usr/lib/llvm-13/cmake /usr/lib/cmake/llvm
# fix more lld compilation issues
mkdir ./src/llvm-project/lld/MachO/mach-o
wget -P ./src/llvm-project/lld/MachO/mach-o/ https://raw.githubusercontent.com/llvm-mirror/libunwind/master/include/mach-o/compact_unwind_encoding.h

# modify config.toml (see below)
# ...

./x.py build && ./x.py install

# update environment (see below)
# ...

# restart you shell and rust should be ready
```

### config.toml

```toml
# config.toml
profile = "user"
changelog-seen = 2

[install]
prefix = "/home/ubuntu/.rustup/toolchains/dev"
sysconfdir = "/home/ubuntu/.rustup/toolchains/dev"

[rust]
lld = true
codegen-tests = false # set to false to speed up building

[target.x86_64-unknown-linux-gnu]
llvm-config = "/usr/bin/llvm-config-13"
llvm-filecheck = "/usr/bin/FileCheck-13"
```

### Environment

```
export PATH="/home/ubuntu/.rustup/toolchains/dev/bin/:$PATH"
export PATH="/home/ubuntu/.cargo/bin/:$PATH"
```

