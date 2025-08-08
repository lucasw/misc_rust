
```
openocd -f ../nucleo_postcard/openocd_std32h753.cfg
```

```
gdb-multiarch -x ../nucleo_postcard/openocd.gdb -q target/thumbv7em-none-eabihf/debug/nucleo_embassy
```
