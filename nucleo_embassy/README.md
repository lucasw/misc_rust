
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


Check ntp server status

```
ntpdate -q 192.168.0.100
```

(or whatever the ip address is, this should be the same as what cargo_build.sh passes to build.rs)


## chrony

serverstats will show ntp packets getting received:

```
sudo chronyc serverstats
NTP packets received       : 76
NTP packets dropped        : 0
Command packets received   : 17
Command packets dropped    : 0
Client log records dropped : 0
NTS-KE connections accepted: 0
NTS-KE connections dropped : 0
Authenticated NTP packets  : 0
Interleaved NTP packets    : 0
NTP timestamps held        : 0
NTP timestamp span         : 0
NTP daemon RX timestamps   : 0
NTP daemon TX timestamps   : 76
NTP kernel RX timestamps   : 76
NTP kernel TX timestamps   : 0
NTP hardware RX timestamps : 0
NTP hardware TX timestamps : 0
```

Use chrony clientloglimit to view individual requests in `/etc/chrony/chrony.conf`:

```
# Uncomment the following line to turn logging on.
log tracking measurements statistics
clientloglimit 1048576
```

```
sudo chronyc clients
Hostname                      NTP   Drop Int IntL Last     Cmd   Drop Int  Last
===============================================================================
localhost                       0      0   -   -     -       4      0   2    23
...
192.168.0.123                   2      0   2   -     0       0      0   -     -
```
