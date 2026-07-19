;; ============================================================
;; Shadowlynx ProX — Offensive Security Plugin
;; ============================================================
;; WebAssembly Text Format (WAT) plugin
;;
;; Tools:
;;   tool_ping          - connectivity check, returns timestamp
;;   tool_http_headers  - uses slpx.http_get
;;   tool_port_scan     - enumerates 8 common ports
;;   tool_subnet_enum   - generates a 16-host subnet list
;;   tool_dns_resolve   - simulated DNS records
;;   tool_version       - returns plugin version info
;;
;; Required exports:
;;   memory                       - linear memory
;;   alloc(i32) -> i32            - bump allocator
;;   tool_<name>(i32,i32) -> i64  - returns packed (ptr<<32)|len
;;
;; Host imports (namespace "slpx"):
;;   log(level: i32, ptr: i32, len: i32)
;;   http_get(url_ptr: i32, url_len: i32, out_ptr: i32) -> i32
;;   get_time() -> i64
;;
;; Memory layout:
;;   0..1023   = scratch (zeroed)
;;   1024..   = read-only string table (data segments)
;;   4096..   = heap (bump-allocator workspace)
;; ============================================================

(module $offensive_plugin
  (import "slpx" "log"      (func $slpx_log      (param i32 i32 i32)))
  (import "slpx" "http_get" (func $slpx_http_get (param i32 i32 i32) (result i32)))
  (import "slpx" "get_time" (func $slpx_get_time (result i64)))

  (memory (export "memory") 8)   ;; 8 pages = 512 KiB

  (global $heap (mut i32) (i32.const 4096))

  ;; alloc(n) -> ptr
  (func $alloc (export "alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p
      (i32.and
        (i32.add (global.get $heap) (i32.const 7))
        (i32.const -8)))
    (global.set $heap (i32.add (local.get $p) (local.get $n)))
    (local.get $p))

  ;; str_copy(dst, off, src, len) -> new_off
  (func $str_copy (param $dst i32) (param $off i32) (param $src i32) (param $len i32) (result i32)
    (local $i i32)
    (local.set $i (i32.const 0))
    (block $done
      (loop $lp
        (br_if $done (i32.ge_s (local.get $i) (local.get $len)))
        (i32.store8
          (i32.add (local.get $dst) (local.get $off))
          (i32.load8_u (i32.add (local.get $src) (local.get $i))))
        (local.set $off (i32.add (local.get $off) (i32.const 1)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $lp)))
    (local.get $off))

  ;; str_const(dst, off, data_offset, len) -> new_off
  (func $str_const (param $dst i32) (param $off i32) (param $src i32) (param $len i32) (result i32)
    (call $str_copy (local.get $dst) (local.get $off) (local.get $src) (local.get $len)))

  ;; pack(ptr, len) -> i64
  (func $pack (param $ptr i32) (param $len i32) (result i64)
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32))
      (i64.extend_i32_u (local.get $len))))

  ;; write_u32(buf, off, n) -> new_off
  (func $write_u32 (param $buf i32) (param $off i32) (param $n i32) (result i32)
    (local $top i32)
    (local $digit i32)
    (local $tmp i32)
    (local $i i32)

    (if (i32.eqz (local.get $n))
      (then
        (i32.store8 (i32.add (local.get $buf) (local.get $off)) (i32.const 48))
        (return (i32.add (local.get $off) (i32.const 1)))))

    (block $gen_done
      (loop $gen
        (br_if $gen_done (i32.eqz (local.get $n)))
        (local.set $digit (i32.add (i32.const 48) (i32.rem_u (local.get $n) (i32.const 10))))
        (i32.store8 (i32.add (local.get $buf) (local.get $off) (local.get $top)) (local.get $digit))
        (local.set $top (i32.add (local.get $top) (i32.const 1)))
        (local.set $n (i32.div_u (local.get $n) (i32.const 10)))
        (br $gen)))

    (local.set $i (i32.const 0))
    (block $rev_done
      (loop $rev
        (local.set $tmp (i32.sub (i32.sub (local.get $top) (local.get $i)) (i32.const 1)))
        (br_if $rev_done (i32.le_s (local.get $tmp) (local.get $i)))
        (local.set $digit
          (i32.load8_u (i32.add (local.get $buf) (local.get $off) (local.get $i))))
        (i32.store8
          (i32.add (local.get $buf) (local.get $off) (local.get $i))
          (i32.load8_u (i32.add (local.get $buf) (local.get $off) (local.get $tmp))))
        (i32.store8
          (i32.add (local.get $buf) (local.get $off) (local.get $tmp))
          (local.get $digit))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $rev)))

    (i32.add (local.get $off) (local.get $top)))

  ;; write_i64(buf, off, n) -> new_off
  (func $write_i64 (param $buf i32) (param $off i32) (param $n i64) (result i32)
    (local $top i32)
    (local $digit i32)
    (local $tmp i32)
    (local $i i32)
    (local $n32 i32)

    (if (i64.eqz (local.get $n))
      (then
        (i32.store8 (i32.add (local.get $buf) (local.get $off)) (i32.const 48))
        (return (i32.add (local.get $off) (i32.const 1)))))

    (block $gen_done
      (loop $gen
        (br_if $gen_done (i64.eqz (local.get $n)))
        (local.set $n32 (i32.wrap_i64 (local.get $n)))
        (local.set $digit (i32.add (i32.const 48) (i32.rem_u (local.get $n32) (i32.const 10))))
        (i32.store8 (i32.add (local.get $buf) (local.get $off) (local.get $top)) (local.get $digit))
        (local.set $top (i32.add (local.get $top) (i32.const 1)))
        (local.set $n (i64.div_u (local.get $n) (i64.const 10)))
        (br $gen)))

    (local.set $i (i32.const 0))
    (block $rev_done
      (loop $rev
        (local.set $tmp (i32.sub (i32.sub (local.get $top) (local.get $i)) (i32.const 1)))
        (br_if $rev_done (i32.le_s (local.get $tmp) (local.get $i)))
        (local.set $digit
          (i32.load8_u (i32.add (local.get $buf) (local.get $off) (local.get $i))))
        (i32.store8
          (i32.add (local.get $buf) (local.get $off) (local.get $i))
          (i32.load8_u (i32.add (local.get $buf) (local.get $off) (local.get $tmp))))
        (i32.store8
          (i32.add (local.get $buf) (local.get $off) (local.get $tmp))
          (local.get $digit))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $rev)))

    (i32.add (local.get $off) (local.get $top)))

  ;; ============================================================
  ;; TOOL: tool_ping(target)
  ;; ============================================================
  (func $tool_ping (export "tool_ping") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local $t i64)
    (local.set $buf (call $alloc (i32.const 512)))
    (local.set $off (i32.const 0))
    (local.set $t (call $slpx_get_time))

    ;; {"success":true,"target":"     (26 chars @ 1024)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1024) (i32.const 26)))
    (local.set $off
      (call $str_copy (local.get $buf) (local.get $off) (local.get $arg_ptr)
        (select (local.get $arg_len) (i32.const 96) (i32.lt_s (local.get $arg_len) (i32.const 96)))))
    ;; ","rtt_ms":42,"timestamp":     (26 chars @ 1050)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1050) (i32.const 26)))
    (local.set $off (call $write_i64 (local.get $buf) (local.get $off) (local.get $t)))
    ;; "}                              (2 chars @ 1076)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1076) (i32.const 2)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_http_headers(url)
  ;; ============================================================
  (func $tool_http_headers (export "tool_http_headers") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $out i32)
    (local $off i32)
    (local $resp_len i32)
    (local.set $buf (call $alloc (i32.const 4096)))
    (local.set $out (call $alloc (i32.const 2048)))
    (local.set $off (i32.const 0))

    (local.set $resp_len
      (call $slpx_http_get (local.get $arg_ptr) (local.get $arg_len) (local.get $out)))

    ;; {"tool":"http_headers","ok":true,"bytes":     (41 chars @ 1078)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1078) (i32.const 41)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $resp_len)))
    ;; ,"preview":"                                  (12 chars @ 1119)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1119) (i32.const 12)))
    (local.set $off
      (call $str_copy (local.get $buf) (local.get $off) (local.get $out)
        (select (local.get $resp_len) (i32.const 96) (i32.lt_s (local.get $resp_len) (i32.const 96)))))
    ;; "}
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1076) (i32.const 2)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_port_scan(target)
  ;; ============================================================
  (func $tool_port_scan (export "tool_port_scan") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local.set $buf (call $alloc (i32.const 4096)))
    (local.set $off (i32.const 0))

    ;; {"tool":"port_scan","target":"    (30 chars @ 1131)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1131) (i32.const 30)))
    (local.set $off
      (call $str_copy (local.get $buf) (local.get $off) (local.get $arg_ptr)
        (select (local.get $arg_len) (i32.const 64) (i32.lt_s (local.get $arg_len) (i32.const 64)))))
    ;; ","results":[                    (13 chars @ 1161)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1161) (i32.const 13)))
    ;; 8 port entries @ 1174, 1221, 1265, 1317, 1368, 1424, 1480, 1529
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1174) (i32.const 47)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1221) (i32.const 44)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1265) (i32.const 46)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1317) (i32.const 52)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1368) (i32.const 51)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1424) (i32.const 56)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1480) (i32.const 49)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1529) (i32.const 56)))
    ;; ]}    (2 chars @ 1585)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1585) (i32.const 2)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_subnet_enum(base)
  ;; ============================================================
  (func $tool_subnet_enum (export "tool_subnet_enum") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local $i i32)
    (local $oct i32)
    (local $h i32)
    (local $t i32)
    (local $o i32)
    (local.set $buf (call $alloc (i32.const 8192)))
    (local.set $off (i32.const 0))

    ;; {"tool":"subnet_enum","hosts":[  (31 chars @ 1587)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1587) (i32.const 31)))

    (local.set $i (i32.const 1))
    (block $done
      (loop $lp
        (br_if $done (i32.gt_s (local.get $i) (i32.const 16)))
        (if (i32.gt_s (local.get $i) (i32.const 1))
          (then (i32.store8 (i32.add (local.get $buf) (local.get $off)) (i32.const 44))
                (local.set $off (i32.add (local.get $off) (i32.const 1)))))
        ;; {"ip":"10.0.0.                  (14 chars @ 1618)
        (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1618) (i32.const 14)))
        (local.set $oct (local.get $i))
        (local.set $h (i32.div_s (local.get $oct) (i32.const 100)))
        (local.set $t (i32.rem_s (i32.div_s (local.get $oct) (i32.const 10)) (i32.const 10)))
        (local.set $o (i32.rem_s (local.get $oct) (i32.const 10)))
        (if (i32.gt_s (local.get $h) (i32.const 0))
          (then (i32.store8 (i32.add (local.get $buf) (local.get $off)) (i32.add (i32.const 48) (local.get $h)))
                (local.set $off (i32.add (local.get $off) (i32.const 1)))))
        (if (i32.gt_s (local.get $t) (i32.const 0))
          (then (i32.store8 (i32.add (local.get $buf) (local.get $off)) (i32.add (i32.const 48) (local.get $t)))
                (local.set $off (i32.add (local.get $off) (i32.const 1)))))
        (i32.store8 (i32.add (local.get $buf) (local.get $off)) (i32.add (i32.const 48) (local.get $o)))
        (local.set $off (i32.add (local.get $off) (i32.const 1)))
        ;; "}
        (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1076) (i32.const 2)))

        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $lp)))

    ;; ],"count":16}    (13 chars @ 1632)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1632) (i32.const 13)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_dns_resolve(target)
  ;; ============================================================
  (func $tool_dns_resolve (export "tool_dns_resolve") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local.set $buf (call $alloc (i32.const 1024)))
    (local.set $off (i32.const 0))

    ;; {"tool":"dns_resolve","target":"  (32 chars @ 1645)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1645) (i32.const 32)))
    (local.set $off
      (call $str_copy (local.get $buf) (local.get $off) (local.get $arg_ptr)
        (select (local.get $arg_len) (i32.const 64) (i32.lt_s (local.get $arg_len) (i32.const 64)))))
    ;; ","records":[                    (13 chars @ 1677)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1677) (i32.const 13)))
    ;; A record      (36 chars @ 1690)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1690) (i32.const 36)))
    ;; AAAA record   (39 chars @ 1726)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1726) (i32.const 39)))
    ;; MX record     (42 chars @ 1765)
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1765) (i32.const 42)))
    ;; ]}
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1585) (i32.const 2)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_version
  ;; ============================================================
  (func $tool_version (export "tool_version") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local.set $buf (call $alloc (i32.const 256)))
    (call $slpx_log (i32.const 0) (i32.const 1807) (i32.const 68))
    (call $pack (local.get $buf) (i32.const 0)))

  ;; ============================================================
  ;; DATA SEGMENTS — Sequential, no overlaps
  ;; Layout: 0..1023   = scratch
  ;;         1024..1874 = string table (851 bytes)
  ;;         4096..     = heap
  ;; ============================================================
  (data (i32.const 1024) "{\"success\":true,\"target\":\"")               ;; 26 @1024, ends 1050
  (data (i32.const 1050) "\",\"rtt_ms\":42,\"timestamp\":")              ;; 26 @1050, ends 1076
  (data (i32.const 1076) "\"}")                                         ;;  2 @1076, ends 1078
  (data (i32.const 1078) "{\"tool\":\"http_headers\",\"ok\":true,\"bytes\":") ;; 41 @1078, ends 1119
  (data (i32.const 1119) ",\"preview\":\"")                              ;; 12 @1119, ends 1131
  (data (i32.const 1131) "{\"tool\":\"port_scan\",\"target\":\"")         ;; 30 @1131, ends 1161
  (data (i32.const 1161) "\",\"results\":[")                             ;; 13 @1161, ends 1174
  (data (i32.const 1174) "{\"port\":22,\"service\":\"ssh\",\"state\":\"filtered\"},")         ;; 47 @1174
  (data (i32.const 1221) "{\"port\":80,\"service\":\"http\",\"state\":\"open\"},")            ;; 44 @1221
  (data (i32.const 1265) "{\"port\":443,\"service\":\"https\",\"state\":\"open\"},")         ;; 46 @1265
  (data (i32.const 1311) "{\"port\":8080,\"service\":\"http-alt\",\"state\":\"closed\"},")   ;; 52 @1311 (with comma)
  (data (i32.const 1368) "{\"port\":3306,\"service\":\"mysql\",\"state\":\"filtered\"},")    ;; 51 @1368 (with comma)
  (data (i32.const 1424) "{\"port\":5432,\"service\":\"postgresql\",\"state\":\"filtered\"},") ;; 56 @1424 (with comma)
  (data (i32.const 1480) "{\"port\":6379,\"service\":\"redis\",\"state\":\"closed\"},")     ;; 49 @1480 (with comma)
  (data (i32.const 1529) "{\"port\":9200,\"service\":\"elasticsearch\",\"state\":\"closed\"}") ;; 56 @1529 (no trailing comma)
  (data (i32.const 1585) "]}")                                          ;;  2 @1585
  (data (i32.const 1587) "{\"tool\":\"subnet_enum\",\"hosts\":[")         ;; 31 @1587
  (data (i32.const 1618) "{\"ip\":\"10.0.0.")                            ;; 14 @1618
  (data (i32.const 1632) "],\"count\":16}")                              ;; 13 @1632
  (data (i32.const 1645) "{\"tool\":\"dns_resolve\",\"target\":\"")       ;; 32 @1645
  (data (i32.const 1677) "\",\"records\":[")                             ;; 13 @1677
  (data (i32.const 1690) "{\"type\":\"A\",\"value\":\"203.0.113.42\"},")  ;; 36 @1690 (with comma)
  (data (i32.const 1726) "{\"type\":\"AAAA\",\"value\":\"2001:db8::42\"},") ;; 39 @1726 (with comma)
  (data (i32.const 1765) "{\"type\":\"MX\",\"value\":\"mail.shadowlynx.io\"}") ;; 42 @1765
  (data (i32.const 1807) "offensive-security plugin v1.0.0 - Shadowlynx ProX offensive toolkit") ;; 68 @1807
)

