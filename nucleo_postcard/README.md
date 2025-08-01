Send and receive messages from no_std stm32 nucleo board to a fully std computer running an application in net_loopback.

Edit the remote and local ip addresses to be on the same subnet as each other and the computer you have connected to the nucleo board (make sure IP_LOCAL isn't currently used by anything else on your local network):

```
const IP_LOCAL: [u8; 4] = [192, 168, 0, 123];
const IP_REMOTE: [u8; 4] = [192, 168, 0, 255];
```

build the debug binary:

```
cargo build
```

```
openocd -f openocd_std32h753.cfg
```

```
Open On-Chip Debugger 0.12.0
Licensed under GNU GPL v2
For bug reports, read
	http://openocd.org/doc/doxygen/bugs.html
Info : The selected transport took over low-level target control. The results might differ compared to plain JTAG/SWD
srst_only separate srst_nogate srst_open_drain connect_deassert_srst

Info : Listening on port 6666 for tcl connections
Info : Listening on port 4444 for telnet connections
Info : clock speed 1800 kHz
Info : STLINK V3J5M2 (API v3) VID:PID 0483:374E
Info : Target voltage: 3.287060
Info : [STM32H753ZIT6.cpu0] Cortex-M7 r1p1 processor detected
Info : [STM32H753ZIT6.cpu0] target has 8 breakpoints, 4 watchpoints
Info : starting gdb server for STM32H753ZIT6.cpu0 on 3333
Info : Listening on port 3333 for gdb connections
Info : accepting 'gdb' connection on tcp/3333
[STM32H753ZIT6.cpu0] halted due to debug-request, current mode: Handler HardFault
xPSR: 0x01000003 pc: 0x0800e208 msp: 0x2001f100
Info : Device: STM32H74x/75x
Info : flash size probed value 2048k
Info : STM32H7 flash has dual banks
Info : Bank (0) size is 1024 kb, base address is 0x08000000
Info : Device: STM32H74x/75x
Info : flash size probed value 2048k
Info : STM32H7 flash has dual banks
Info : Bank (1) size is 1024 kb, base address is 0x08100000
```

## gdb

```
gdb-multiarch -x openocd.gdb -q target/thumbv7em-none-eabihf/debug/postcard_rx_tx
```

```
Reading symbols from target/thumbv7em-none-eabihf/debug/postcard_rx_tx...
0x0800e208 in stm32h7xx_hal::rcc::{impl#3}::freeze::{closure#3} (w=0x4 <core::ptr::drop_in_place<&smoltcp::wire::ipv6::Address>+4>) at src/rcc/mod.rs:657
657	        mut self,
Breakpoint 1 at 0x802c07a: file src/lib.rs, line 1112.
Note: automatically using hardware breakpoints for read-only addresses.
Breakpoint 2 at 0x802ec00: file src/lib.rs, line 1105.
Breakpoint 3 at 0x8001084: file src/lib.rs, line 32.
semihosting is enabled
```

load the built binary (can cargo build and rerun this command)

```
(gdb) load
Loading section .vector_table, size 0x298 lma 0x8000000
Loading section .text, size 0x2e96c lma 0x8000298
Loading section .rodata, size 0x77b4 lma 0x802ec08
Loading section .data, size 0x618 lma 0x80363c0
Start address 0x08000298, load size 223696
Transfer rate: 57 KB/sec, 13981 bytes/write.
```

```
(gdb) run
The program being debugged has been started already.
Start it from the beginning? (y or n) y
Starting program: /home/lucasw/rust/misc/misc_rust/nucleo_postcard/target/thumbv7em-none-eabihf/debug/postcard_rx_tx
```

openocd output:

```
[STM32H753ZIT6.cpu0] halted due to debug-request, current mode: Thread
xPSR: 0x01000000 pc: 0x08000298 msp: 0x20020000, semihosting
Setting up board
Bringing up ethernet interface
Waiting for link to come up
Entering main loop
```

## check built binary

```
file target/thumbv7em-none-eabihf/debug/postcard_rx_tx
target/thumbv7em-none-eabihf/debug/postcard_rx_tx: ELF 32-bit LSB executable, ARM, EABI5 version 1 (SYSV), statically linked, with debug_info, not stripped
```

```
cargo readobj --bin postcard_rx_tx -- --file-headers
...
ELF Header:
  Magic:   7f 45 4c 46 01 01 01 00 00 00 00 00 00 00 00 00
  Class:                             ELF32
  Data:                              2's complement, little endian
  Version:                           1 (current)
  OS/ABI:                            UNIX - System V
  ABI Version:                       0
  Type:                              EXEC (Executable file)
  Machine:                           ARM
  Version:                           0x1
  Entry point address:               0x8000299
  Start of program headers:          52 (bytes into file)
  Start of section headers:          5425556 (bytes into file)
  Flags:                             0x5000400
  Size of this header:               52 (bytes)
  Size of program headers:           32 (bytes)
  Number of program headers:         7
  Size of section headers:           40 (bytes)
  Number of section headers:         28
  Section header string table index: 26
```
