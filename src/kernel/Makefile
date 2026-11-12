# 混合内核构建脚本
# Rust no_std 内核 + 少量汇编

TARGET = x86_64-unknown-none
MODE ?= release
KERNEL_ELF = target/$(TARGET)/$(MODE)/hybrid_kernel
KERNEL_BIN = kernel.bin
BOOT_OBJ = build/boot_entry.o
SYSCALL_OBJ = build/syscall_stub.o

NASM = nasm
LD = ld.lld
CARGO = cargo

.PHONY: all clean kernel asm user run

all: kernel

# 汇编编译
asm:
	@mkdir -p build
	$(NASM) -f elf64 src/asm/boot_entry.s -o $(BOOT_OBJ)
	$(NASM) -f elf64 src/asm/syscall_stub.s -o $(SYSCALL_OBJ)

# 内核编译
kernel: asm
	$(CARGO) build --target $(TARGET) --$(MODE) --lib
	$(LD) -n -T linker.ld -o $(KERNEL_ELF) \
		$(BOOT_OBJ) $(SYSCALL_OBJ) \
		target/$(TARGET)/$(MODE)/libhybrid_kernel.a
	@echo "Kernel built: $(KERNEL_ELF)"

# 用户态 init
user:
	$(CARGO) build --target $(TARGET) --$(MODE) --bin init

# 清理
clean:
	$(CARGO) clean
	rm -rf build $(KERNEL_BIN)

# 显示大小
size: kernel
	@size $(KERNEL_ELF) 2>/dev/null || echo "size tool not available"
