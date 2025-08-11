#!/bin/sh
gdb-multiarch -x ../nucleo_postcard/openocd.gdb -q target/thumbv7em-none-eabihf/debug/nucleo_embassy
