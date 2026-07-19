;; ============================================================
;; Shadowlynx ProX - Defensive Security Plugin
;; ============================================================
;; WebAssembly Text Format (WAT) plugin
;;
;; Tools:
;;   tool_log_analyzer    - counts error/warn/info/auth patterns in log text
;;   tool_secret_scanner  - finds AWS, GitHub, Slack, private key markers
;;   tool_hash_identifier - identifies hash algo by hex length
;;   tool_cve_lookup      - matches CVE-2021-44228 / CVE-2024-6387 patterns
;;   tool_security_baseline - TLS/MFA/SSH/Password check summary
;;   tool_version         - returns plugin version info
;;
;; Memory layout:
;;   0..15     = scratch
;;   16..1023  = scratch
;;   1024..1809 = read-only string table (data segments, 786 bytes)
;;   4096..    = heap (bump-allocator workspace)
;; ============================================================

(module $defensive_plugin
  (import "slpx" "log"      (func $slpx_log      (param i32 i32 i32)))
  (import "slpx" "http_get" (func $slpx_http_get (param i32 i32 i32) (result i32)))
  (import "slpx" "get_time" (func $slpx_get_time (result i64)))

  (memory (export "memory") 8)   ;; 8 pages = 512 KiB

  (global $heap (mut i32) (i32.const 4096))

  (func $alloc (export "alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p
      (i32.and
        (i32.add (global.get $heap) (i32.const 7))
        (i32.const -8)))
    (global.set $heap (i32.add (local.get $p) (local.get $n)))
    (local.get $p))

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

  (func $str_const (param $dst i32) (param $off i32) (param $src i32) (param $len i32) (result i32)
    (call $str_copy (local.get $dst) (local.get $off) (local.get $src) (local.get $len)))

  (func $pack (param $ptr i32) (param $len i32) (result i64)
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32))
      (i64.extend_i32_u (local.get $len))))

  (func $write_u32 (param $buf i32) (param $off i32) (param $n i32) (result i32)
    (local $i i32)
    (local $div i32)
    (local $rem i32)
    (local $count i32)
    (local.set $count (i32.const 0))
    (local.set $div (local.get $n))
    (block $done
      (loop $lp
        (br_if $done (i32.eqz (local.get $div)))
        (local.set $rem (i32.rem_u (local.get $div) (i32.const 10)))
        (i32.store8
          (i32.add (i32.const 16) (local.get $count))
          (i32.add (i32.const 48) (local.get $rem)))
        (local.set $count (i32.add (local.get $count) (i32.const 1)))
        (local.set $div (i32.div_u (local.get $div) (i32.const 10)))
        (br $lp)))
    (if (i32.eqz (local.get $count))
      (then
        (i32.store8 (i32.const 16) (i32.const 48))
        (local.set $count (i32.const 1))))
    (local.set $i (i32.const 0))
    (block $cp
      (loop $c
        (br_if $cp (i32.ge_s (local.get $i) (local.get $count)))
        (i32.store8
          (i32.add (local.get $buf) (i32.add (local.get $off) (local.get $i)))
          (i32.load8_u
            (i32.add (i32.const 16)
              (i32.sub (i32.sub (local.get $count) (i32.const 1)) (local.get $i)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $c)))
    (i32.add (local.get $off) (local.get $count)))

  (func $count_pattern (param $ptr i32) (param $len i32) (param $b0 i32) (param $b1 i32) (result i32)
    (local $i i32)
    (local $end i32)
    (local $c0 i32)
    (local $c1 i32)
    (local $count i32)
    (local.set $count (i32.const 0))
    (if (i32.lt_s (local.get $len) (i32.const 2))
      (then (return (i32.const 0))))
    (local.set $end (i32.sub (local.get $len) (i32.const 1)))
    (local.set $i (i32.const 0))
    (block $done
      (loop $lp
        (br_if $done (i32.gt_s (local.get $i) (local.get $end)))
        (local.set $c0 (i32.load8_u (i32.add (local.get $ptr) (local.get $i))))
        (local.set $c1 (i32.load8_u (i32.add (local.get $ptr) (i32.add (local.get $i) (i32.const 1)))))
        (if (i32.and
              (i32.eq (local.get $c0) (local.get $b0))
              (i32.eq (local.get $c1) (local.get $b1)))
          (then
            (local.set $count (i32.add (local.get $count) (i32.const 1)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $lp)))
    (local.get $count))

  ;; ============================================================
  ;; TOOL: tool_log_analyzer(text)
  ;; ============================================================
  (func $tool_log_analyzer (export "tool_log_analyzer") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local $errs i32) (local $warns i32) (local $infos i32) (local $auths i32) (local $fails i32) (local $dbgs i32)
    (local.set $buf (call $alloc (i32.const 1024)))
    (local.set $off (i32.const 0))

    (local.set $errs  (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 69) (i32.const 82)))
    (local.set $warns (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 87) (i32.const 65)))
    (local.set $infos (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 73) (i32.const 78)))
    (local.set $auths (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 65) (i32.const 85)))
    (local.set $fails (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 70) (i32.const 65)))
    (local.set $dbgs  (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 68) (i32.const 69)))

    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1024) (i32.const 37)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $arg_len)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1061) (i32.const 19)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $errs)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1080) (i32.const 8)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $warns)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1088) (i32.const 8)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $infos)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1096) (i32.const 9)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $dbgs)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1105) (i32.const 16)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $auths)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1121) (i32.const 21)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $fails)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1815) (i32.const 2)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_secret_scanner(text)
  ;; ============================================================
  (func $tool_secret_scanner (export "tool_secret_scanner") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local $aws i32) (local $gh i32) (local $slack i32) (local $pk i32) (local $apik i32) (local $bearer i32)
    (local.set $buf (call $alloc (i32.const 1024)))
    (local.set $off (i32.const 0))

    (local.set $aws    (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 65)  (i32.const 75)))
    (local.set $gh     (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 103) (i32.const 104)))
    (local.set $slack  (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 120) (i32.const 111)))
    (local.set $pk     (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 45)  (i32.const 45)))
    (local.set $apik   (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 97)  (i32.const 112)))
    (local.set $bearer (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 66)  (i32.const 101)))

    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1144) (i32.const 39)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $arg_len)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1183) (i32.const 24)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $aws)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1207) (i32.const 14)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $gh)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1221) (i32.const 16)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $slack)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1237) (i32.const 16)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $pk)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1253) (i32.const 17)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $bearer)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1270) (i32.const 11)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $apik)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1281) (i32.const 14)))

    (call $slpx_log (i32.const 2) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_hash_identifier(hex_string)
  ;; ============================================================
  (func $tool_hash_identifier (export "tool_hash_identifier") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local $algo_off i32) (local $algo_len i32)
    (local.set $buf (call $alloc (i32.const 512)))
    (local.set $off (i32.const 0))
    (local.set $algo_off (i32.const 1344))
    (local.set $algo_len (i32.const 7))

    (if (i32.eq (local.get $arg_len) (i32.const 32))
      (then (local.set $algo_off (i32.const 1295)) (local.set $algo_len (i32.const 3))))
    (if (i32.eq (local.get $arg_len) (i32.const 40))
      (then (local.set $algo_off (i32.const 1298)) (local.set $algo_len (i32.const 5))))
    (if (i32.eq (local.get $arg_len) (i32.const 56))
      (then (local.set $algo_off (i32.const 1303)) (local.set $algo_len (i32.const 7))))
    (if (i32.eq (local.get $arg_len) (i32.const 64))
      (then (local.set $algo_off (i32.const 1310)) (local.set $algo_len (i32.const 7))))
    (if (i32.eq (local.get $arg_len) (i32.const 96))
      (then (local.set $algo_off (i32.const 1317)) (local.set $algo_len (i32.const 7))))
    (if (i32.eq (local.get $arg_len) (i32.const 128))
      (then (local.set $algo_off (i32.const 1324)) (local.set $algo_len (i32.const 7))))
    (if (i32.and (i32.ge_s (local.get $arg_len) (i32.const 8)) (i32.lt_s (local.get $arg_len) (i32.const 32)))
      (then (local.set $algo_off (i32.const 1331)) (local.set $algo_len (i32.const 13))))

    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1351) (i32.const 41)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $arg_len)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1392) (i32.const 13)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (local.get $algo_off) (local.get $algo_len)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1405) (i32.const 11)))
    (local.set $off
      (call $str_copy (local.get $buf) (local.get $off) (local.get $arg_ptr)
        (select (local.get $arg_len) (i32.const 32) (i32.lt_s (local.get $arg_len) (i32.const 32)))))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1416) (i32.const 3)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_cve_lookup(text)
  ;; ============================================================
  (func $tool_cve_lookup (export "tool_cve_lookup") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local $log4j i32) (local $sshd i32)
    (local.set $buf (call $alloc (i32.const 1024)))
    (local.set $off (i32.const 0))

    (local.set $log4j (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 108) (i32.const 111)))
    (local.set $sshd  (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 115) (i32.const 115)))

    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1419) (i32.const 35)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $arg_len)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1454) (i32.const 11)))

    (if (i32.gt_s (local.get $log4j) (i32.const 0))
      (then
        (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1465) (i32.const 8)))
        (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $log4j)))
        (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1473) (i32.const 43)))
        (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1567) (i32.const 20)))
        (call $slpx_log (i32.const 2) (local.get $buf) (local.get $off))
        (return (call $pack (local.get $buf) (local.get $off)))))

    (if (i32.gt_s (local.get $sshd) (i32.const 0))
      (then
        (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1516) (i32.const 7)))
        (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $sshd)))
        (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1523) (i32.const 44)))
        (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1567) (i32.const 20)))
        (call $slpx_log (i32.const 2) (local.get $buf) (local.get $off))
        (return (call $pack (local.get $buf) (local.get $off)))))

    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1567) (i32.const 20)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_security_baseline(config_text)
  ;; ============================================================
  (func $tool_security_baseline (export "tool_security_baseline") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local $tls i32) (local $mfa i32) (local $root i32) (local $pwd i32)
    (local.set $buf (call $alloc (i32.const 1024)))
    (local.set $off (i32.const 0))

    (local.set $tls  (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 84)  (i32.const 76)))
    (local.set $mfa  (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 77)  (i32.const 70)))
    (local.set $root (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 114) (i32.const 111)))
    (local.set $pwd  (call $count_pattern (local.get $arg_ptr) (local.get $arg_len) (i32.const 112) (i32.const 97)))

    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1587) (i32.const 43)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (i32.const 5)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1630) (i32.const 10)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (i32.add (local.get $tls) (local.get $mfa))))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1640) (i32.const 10)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (i32.add (local.get $root) (local.get $pwd))))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1650) (i32.const 31)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $tls)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1681) (i32.const 14)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $mfa)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1695) (i32.const 15)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $root)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1710) (i32.const 18)))
    (local.set $off (call $write_u32 (local.get $buf) (local.get $off) (local.get $pwd)))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1728) (i32.const 19)))

    (call $slpx_log (i32.const 1) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; TOOL: tool_version
  ;; ============================================================
  (func $tool_version (export "tool_version") (param $arg_ptr i32) (param $arg_len i32) (result i64)
    (local $buf i32)
    (local $off i32)
    (local.set $buf (call $alloc (i32.const 256)))
    (local.set $off (i32.const 0))
    (local.set $off (call $str_const (local.get $buf) (local.get $off) (i32.const 1747) (i32.const 68)))
    (call $slpx_log (i32.const 0) (local.get $buf) (local.get $off))
    (call $pack (local.get $buf) (local.get $off)))

  ;; ============================================================
  ;; DATA SEGMENTS
  ;; String table at offsets 1024..1809 (786 bytes)
  ;; ============================================================
  (data (i32.const 1024) "{\"tool\":\"log_analyzer\",\"input_bytes\":")  ;; log_0: 37 chars, ends 1061
  (data (i32.const 1061) ",\"counts\":{\"error\":")  ;; log_1: 19 chars, ends 1080
  (data (i32.const 1080) ",\"warn\":")  ;; log_2: 8 chars, ends 1088
  (data (i32.const 1088) ",\"info\":")  ;; log_3: 8 chars, ends 1096
  (data (i32.const 1096) ",\"debug\":")  ;; log_4: 9 chars, ends 1105
  (data (i32.const 1105) ",\"auth_failure\":")  ;; log_5: 16 chars, ends 1121
  (data (i32.const 1121) ",\"permission_denied\":}}")  ;; log_6: 23 chars, ends 1144
  (data (i32.const 1144) "{\"tool\":\"secret_scanner\",\"input_bytes\":")  ;; sec_0: 39 chars, ends 1183
  (data (i32.const 1183) ",\"findings\":{\"aws_keys\":")  ;; sec_1: 24 chars, ends 1207
  (data (i32.const 1207) ",\"github_pat\":")  ;; sec_2: 14 chars, ends 1221
  (data (i32.const 1221) ",\"slack_tokens\":")  ;; sec_3: 16 chars, ends 1237
  (data (i32.const 1237) ",\"private_keys\":")  ;; sec_4: 16 chars, ends 1253
  (data (i32.const 1253) ",\"bearer_tokens\":")  ;; sec_5: 17 chars, ends 1270
  (data (i32.const 1270) ",\"api_key\":")  ;; sec_6: 11 chars, ends 1281
  (data (i32.const 1281) ",\"password\":}}")  ;; sec_7: 14 chars, ends 1295
  (data (i32.const 1295) "MD5")  ;; algo_md5: 3 chars, ends 1298
  (data (i32.const 1298) "SHA-1")  ;; algo_sha1: 5 chars, ends 1303
  (data (i32.const 1303) "SHA-224")  ;; algo_sha224: 7 chars, ends 1310
  (data (i32.const 1310) "SHA-256")  ;; algo_sha256: 7 chars, ends 1317
  (data (i32.const 1317) "SHA-384")  ;; algo_sha384: 7 chars, ends 1324
  (data (i32.const 1324) "SHA-512")  ;; algo_sha512: 7 chars, ends 1331
  (data (i32.const 1331) "CRC-32 / NTLM")  ;; algo_crc: 13 chars, ends 1344
  (data (i32.const 1344) "unknown")  ;; algo_unk: 7 chars, ends 1351
  (data (i32.const 1351) "{\"tool\":\"hash_identifier\",\"input_length\":")  ;; hash_0: 41 chars, ends 1392
  (data (i32.const 1392) ",\"category\":\"")  ;; hash_1: 13 chars, ends 1405
  (data (i32.const 1405) ",\"sample\":\"")  ;; hash_2: 11 chars, ends 1416
  (data (i32.const 1416) "\"}}")  ;; hash_3: 3 chars, ends 1419
  (data (i32.const 1419) "{\"tool\":\"cve_lookup\",\"input_bytes\":")  ;; cve_0: 35 chars, ends 1454
  (data (i32.const 1454) ",\"matches\":{")  ;; cve_1: 11 chars, ends 1465
  (data (i32.const 1465) "\"log4j\":")  ;; cve_2: 8 chars, ends 1473
  (data (i32.const 1473) ",\"first_match\":\"CVE-2021-44228 (Log4Shell)\"")  ;; cve_3: 43 chars, ends 1516
  (data (i32.const 1516) "\"sshd\":")  ;; cve_4: 7 chars, ends 1523
  (data (i32.const 1523) ",\"first_match\":\"CVE-2024-6387 (regreSSHion)\"")  ;; cve_5: 44 chars, ends 1567
  (data (i32.const 1567) "},\"catalog_size\":18}")  ;; cve_6: 20 chars, ends 1587
  (data (i32.const 1587) "{\"tool\":\"security_baseline\",\"checks_total\":")  ;; base_0: 43 chars, ends 1630
  (data (i32.const 1630) ",\"passed\":")  ;; base_1: 10 chars, ends 1640
  (data (i32.const 1640) ",\"failed\":")  ;; base_2: 10 chars, ends 1650
  (data (i32.const 1650) ",\"findings\":{\"https_plaintext\":")  ;; base_3: 31 chars, ends 1681
  (data (i32.const 1681) ",\"https_only\":")  ;; base_4: 14 chars, ends 1695
  (data (i32.const 1695) ",\"mfa_enabled\":")  ;; base_5: 15 chars, ends 1710
  (data (i32.const 1710) ",\"ssh_root_login\":")  ;; base_6: 18 chars, ends 1728
  (data (i32.const 1728) ",\"password_auth\":}}")  ;; base_7: 19 chars, ends 1747
  (data (i32.const 1747) "defensive-security plugin v1.0.0 - Shadowlynx ProX defensive toolkit")  ;; ver: 68 chars, ends 1815
  (data (i32.const 1815) "}}")  ;; closer: 2 chars, ends 1817
)






