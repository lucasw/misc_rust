
```
openocd -f ../nucleo_postcard/openocd_std32h753.cfg
```

```
gdb-multiarch -x ../nucleo_postcard/openocd.gdb -q target/thumbv7em-none-eabihf/debug/nucleo_embassy
```


```
nc -ul 34200 | hexdump
```

```
echo "test" | nc -u 192.168.0.123 34201
```
