# Build difference: 01.13 -> 01.24

- Types: 4942 in 01.13, 5041 in 01.24; 4858 structurally identical, 59 changed, 25 removed, 124 added.
- Handles renumbered: 11 of 4917 matched types.

## Handles the stand-in uses

| 01.13 | 01.24 | match | first use |
| --- | --- | --- | --- |
| `0x808004ac` | `0x808004ac` | same | investment.py:16 |
| `0x808004b1` | `0x808004b1` | same | bap_server.py:460 |
| `0x808004fa` | `0x808004fa` | same | bap_server.py:1543 |
| `0x80800646` | `0x80800646` | same | bap_server.py:552 |
| `0x808008a3` | `0x808008a3` | same | bap_server.py:460 |
| `0x808009ed` | `0x808009ed` | same | bap_server.py:457 |
| `0x80800a58` | `0x80800a58` | changed | bap_server.py:383 |
| `0x80800c56` | `0x80800c56` | same | bap_server.py:463 |
| `0x80800c6f` | `0x80800c6f` | same | bap_server.py:722 |
| `0x80800cc5` | `0x80800cc5` | same | bap_server.py:393 |
| `0x80800cf7` | `0x80800cf7` | same | bap_server.py:458 |
| `0x80800d64` | `0x80800d64` | same | bap_server.py:387 |
| `0x80800d82` | `0x80800d82` | same | bap_server.py:465 |
| `0x80800d9f` | `0x80800d9f` | same | bap_server.py:464 |
| `0x80800e76` | `0x80800e76` | same | bap_server.py:391 |
| `0x80800e98` | `0x80800e98` | changed | bap_server.py:381 |
| `0x80800f1a` | `0x80800f1a` | same | bap_server.py:461 |
| `0x808016e0` | `0x808016e0` | same | bap_server.py:113 |
| `0x80801704` | `0x80801704` | same | bap_server.py:107 |
| `0x80801714` | `0x80801714` | changed | bap_server.py:112 |
| `0x80801743` | `0x80801743` | changed | bap_server.py:71 |
| `0x80801774` | `0x80801774` | same | bap_server.py:863 |
| `0x808017fd` | `0x808017fd` | changed | bap_server.py:77 |
| `0x80801803` | `0x80801803` | changed | bap_server.py:68 |
| `0x8080181a` | `0x8080181a` | same | bap_server.py:162 |
| `0x80801825` | `0x80801825` | changed | bap_server.py:815 |
| `0x8080183c` | `0x8080183c` | changed | bap_server.py:106 |
| `0x80801855` | `0x80801855` | changed | bap_server.py:105 |
| `0x8080185c` | `0x8080185c` | same | bap_server.py:840 |
| `0x80801869` | `0x80801869` | same | bap_server.py:868 |
| `0x80801921` | `0x80801921` | same | bap_server.py:76 |
| `0x80801932` | `0x80801932` | same | investment.py:16 |
| `0x80801941` | `0x80801941` | changed | bap_server.py:753 |
| `0x80801951` | `0x80801951` | same | bap_server.py:69 |
| `0x80801954` | `0x80801954` | changed | bap_server.py:70 |
| `0x808022a5` | `0x808022a5` | same | bap_server.py:460 |
| `0x80802a1b` | `0x80802a1b` | same | bap_server.py:590 |
| `0x80804008` | `0x80804008` | same | bap_server.py:406 |
| `0x80804056` | `0x80804056` | same | bap_server.py:406 |
| `0x8080409f` | `0x8080409f` | same | bap_server.py:393 |
| `0x808043a7` | `0x808043a7` | same | bap_server.py:387 |
| `0x80804595` | `0x80804595` | same | bap_server.py:305 |
| `0x808045d9` | `0x808045d9` | changed | bap_server.py:165 |
| `0x808045da` | `0x808045da` | same | bap_server.py:169 |
| `0x808045f0` | `0x808045f0` | same | bap_server.py:764 |
| `0x808046bc` | `0x808046bc` | same | bap_server.py:171 |
| `0x808046e7` | `0x808046e7` | same | bap_server.py:840 |
| `0x808047d8` | `0x808047d8` | same | bap_server.py:306 |

### Their field layouts

Offset, kind and nested type of each field, old | new (a stand-in built for the old layout writes the wrong offsets on the new one).

`0x80800a58` (size 0x1a8) -> `0x80800a58` (size 0x1a8):

```
  +0x0000 struct:0 0x80800e98         | +0x0000 struct:0 0x80800e98
  +0x0100 u8:3                        | +0x0100 u8:3
  +0x0101 struct:0 0x80804056         | +0x0101 struct:0 0x80804056
  +0x0184 u32:32                      | +0x0184 u32:32
  +0x0188 i32:32                      | +0x0188 i32:32
  +0x018c u32:32                      | +0x018c u32:32
  +0x0190 i32:32                      | +0x0190 i32:32
  +0x0194 u8:8                        | +0x0194 u8:8
  +0x0196 u16:16                      | +0x0196 u16:16
  +0x0198 u16:16                      | +0x0198 u16:16
  +0x019c u32:32                      | +0x019c u32:32
  +0x01a0 u32:32                      | +0x01a0 u32:32
  +0x01a4 u16:16                      | +0x01a4 u16:16
```

`0x80800e98` (size 0x100) -> `0x80800e98` (size 0x100):

```
  +0x0000 u32:32                      | +0x0000 u32:32
  +0x0008 u64:64                      | +0x0008 u64:64
  +0x0010 struct:0 0x80800c2d         | +0x0010 struct:0 0x80800c2d
  +0x0038 struct:0 0x80800b1f         | +0x0038 struct:0 0x80800b1f
  +0x00f0 bool:0                      | +0x00f0 bool:0
* +0x00f2 u16:16                      | +0x00f4 u32:17
  +0x00f4 u32:32                      | +0x00f8 u32:32
  +0x00f8 u16:16                      | +0x00fc u16:16
```

`0x80801714` (size 0x6ae8) -> `0x80801714` (size 0x6db8):

```
  +0x0000 u64?:64                     | +0x0000 u64?:64
  +0x0008 struct?:0 0x808017fc        | +0x0008 struct?:0 0x808017fc
  +0x000c struct?:0 0x8080181a        | +0x000c struct?:0 0x8080181a
  +0x0030 struct?:0 0x80804a18        | +0x0030 struct?:0 0x80804a18
  +0x2838 struct?:0 0x808019ff        | +0x2838 struct?:0 0x808019ff
  +0x2900 struct?:0 0x808045da        | +0x2900 struct?:0 0x808045da
  +0x29a0 struct?:0 0x808017b9        | +0x29a0 struct?:0 0x808017b9
  +0x29c0 struct?:0 0x8080193c        | +0x29c0 struct?:0 0x8080193c
  +0x33a0 struct?:0 0x808047a7        | +0x33a0 struct?:0 0x808047a7
  +0x3b98 struct?:0 0x80801755        | +0x3b98 struct?:0 0x80801755
  +0x3bd0 struct?:0 0x80801701        | +0x3bd0 struct?:0 0x80801701
  +0x4e30 struct?:0 0x80804c21        | +0x5000 struct?:0 0x80804c21
  +0x5630 struct?:0 0x80805bc7        | +0x5800 struct?:0 0x80805bc7
* +0x6630 struct?:0 0x80804707        | +0x6800 struct?:0 0x80801082
  +0x6730 struct?:0 0x80801724        | +0x6a00 struct?:0 0x80801724
  +0x6740 struct?:0 0x808018bb        | +0x6a10 struct?:0 0x808018bb
  +0x68d4 struct?:0 0x8080472b        | +0x6ba4 struct?:0 0x8080472b
  +0x6ad4 struct?:0 0x808016f8        | +0x6da4 struct?:0 0x808016f8
```

`0x80801743` (size 0x6ae8) -> `0x80801743` (size 0x6db8):

```
  +0x0000 struct:0 0x80801714         | +0x0000 struct:0 0x80801714
```

`0x808017fd` (size 0x868) -> `0x808017fd` (size 0x908):

```
  +0x0000 u64?:64                     | +0x0000 u64?:64
  +0x0008 struct?:0 0x808017fc        | +0x0008 struct?:0 0x808017fc
  +0x000c struct?:0 0x8080181a        | +0x000c struct?:0 0x8080181a
  +0x0030 struct?:0 0x80801855        | +0x0030 struct?:0 0x80801855
  +0x0858 struct?:0 0x80801923        | +0x08f8 struct?:0 0x80801923
```

`0x80801803` (size 0x5498) -> `0x80801803` (size 0x7050):

```
  +0x0000 struct:0 0x808018c7         | +0x0000 struct:0 0x808018c7
```

`0x80801825` (size 0x2f0) -> `0x80801825` (size 0x3c8):

```
  +0x0000 struct:0 0x80801796         | +0x0000 struct:0 0x80801796
  +0x0008 u64:64                      | +0x0008 u64:64
  +0x0010 u32:32                      | +0x0010 u32:32
  +0x0014 u32:32                      | +0x0014 u32:32
  +0x0018 struct:0 0x80801941         | +0x0018 struct:0 0x80801941
```

`0x8080183c` (size 0x28) -> `0x8080183c` (size 0x30):

```
  +0x0000 u64?:64                     | +0x0000 u64?:64
  +0x0008 i16?:16                     | +0x0008 i16?:16
  +0x000a i16?:16                     | +0x000a i16?:16
  +0x000c i16?:16                     | +0x000c i16?:16
*                                     | +0x0010 struct?:0 0x80801069
  +0x000e struct?:0 0x808046bc        | +0x0018 struct?:0 0x808046bc
```

`0x80801855` (size 0x828) -> `0x80801855` (size 0x8c8):

```
  +0x0000 i32?:32                     | +0x0000 i32?:32
  +0x0004 f32?:0                      | +0x0004 f32?:0
  +0x0008 f32?:0                      | +0x0008 f32?:0
  +0x000c f32?:0                      | +0x000c f32?:0
  +0x0010 struct?:0 0x80805513        | +0x0010 struct?:0 0x80805513
  +0x0164 struct?:0 0x808040d1        | +0x0164 struct?:0 0x808040d1
  +0x01e8 struct?:0 0x808045d9        | +0x01e8 struct?:0 0x808045d9
  +0x0508 struct?:0 0x808045ad        | +0x05a8 struct?:0 0x808045ad
  +0x0588 struct?:0 0x8080599a        | +0x0628 struct?:0 0x8080599a
  +0x05c8 struct?:0 0x808045c4        | +0x0668 struct?:0 0x808045c4
  +0x05e8 struct?:0 0x808045c4        | +0x0688 struct?:0 0x808045c4
  +0x0608 struct?:0 0x808045c4        | +0x06a8 struct?:0 0x808045c4
  +0x0628 struct?:0 0x80804703        | +0x06c8 struct?:0 0x80804703
  +0x06a8 struct?:0 0x80804703        | +0x0748 struct?:0 0x80804703
  +0x0728 struct?:0 0x80804703        | +0x07c8 struct?:0 0x80804703
  +0x07a8 struct?:0 0x80804703        | +0x0848 struct?:0 0x80804703
```

`0x80801941` (size 0x2d8) -> `0x80801941` (size 0x3b0):

```
  +0x0000 u64?:64                     | +0x0000 u64?:64
  +0x0008 u64?:64                     | +0x0008 u64?:64
  +0x0010 struct?:0 0x808045f0        | +0x0010 struct?:0 0x808045f0
  +0x003c struct?:0 0x80804681        | +0x003c struct?:0 0x80804681
  +0x0108 struct?:0 0x80804ee0        | +0x0108 struct?:0 0x80804ee0
* +0x029c struct?:0 0x808047ad        | +0x029c struct?:0 0x808010c6
  +0x02d0 u32?:32                     | +0x02dc u32?:32
*                                     | +0x02e0 struct?:0 0x808010c7
```

`0x80801954` (size 0x8a0) -> `0x80801954` (size 0x940):

```
  +0x0000 u64?:64                     | +0x0000 u64?:64
  +0x0008 struct?:0 0x808017fc        | +0x0008 struct?:0 0x808017fc
  +0x000c struct?:0 0x8080181a        | +0x000c struct?:0 0x8080181a
  +0x0030 struct?:0 0x80801755        | +0x0030 struct?:0 0x80801755
  +0x0068 struct?:0 0x80801855        | +0x0068 struct?:0 0x80801855
  +0x0890 struct?:0 0x80801923        | +0x0930 struct?:0 0x80801923
```

`0x808045d9` (size 0x320) -> `0x808045d9` (size 0x3c0):

```
  +0x0000 struct:0 0x8080183c         | +0x0000 struct:0 0x8080183c
  +0x0028 struct:0 0x8080183c         | +0x0030 struct:0 0x8080183c
  +0x0050 struct:0 0x8080183c         | +0x0060 struct:0 0x8080183c
  +0x0078 struct:0 0x8080183c         | +0x0090 struct:0 0x8080183c
  +0x00a0 struct:0 0x8080183c         | +0x00c0 struct:0 0x8080183c
  +0x00c8 struct:0 0x8080183c         | +0x00f0 struct:0 0x8080183c
  +0x00f0 struct:0 0x8080183c         | +0x0120 struct:0 0x8080183c
  +0x0118 struct:0 0x8080183c         | +0x0150 struct:0 0x8080183c
  +0x0140 struct:0 0x8080183c         | +0x0180 struct:0 0x8080183c
  +0x0168 struct:0 0x8080183c         | +0x01b0 struct:0 0x8080183c
  +0x0190 struct:0 0x8080183c         | +0x01e0 struct:0 0x8080183c
  +0x01b8 struct:0 0x8080183c         | +0x0210 struct:0 0x8080183c
  +0x01e0 struct:0 0x8080183c         | +0x0240 struct:0 0x8080183c
  +0x0208 struct:0 0x8080183c         | +0x0270 struct:0 0x8080183c
  +0x0230 struct:0 0x8080183c         | +0x02a0 struct:0 0x8080183c
  +0x0258 struct:0 0x8080183c         | +0x02d0 struct:0 0x8080183c
  +0x0280 struct:0 0x8080183c         | +0x0300 struct:0 0x8080183c
  +0x02a8 struct:0 0x8080183c         | +0x0330 struct:0 0x8080183c
  +0x02d0 struct:0 0x8080183c         | +0x0360 struct:0 0x8080183c
  +0x02f8 struct:0 0x8080183c         | +0x0390 struct:0 0x8080183c
```


## Changed types

- `0x80803142` -> `0x80803142` (size 0x70 -> 0x90): same fields at new offsets
- `0x80802681` -> `0x80802681` (size 0x100 -> 0x180): same fields; value ranges or array settings changed
- `0x80802b26` -> `0x80802b26` (size 0x300 -> 0x3d0): `u8:3 u8` -> `i32 u8:3 u8 i32`
- `0x80802460` -> `0x80802460` (size 0x2c0 -> 0x340): same fields; value ranges or array settings changed
- `0x80802a67` -> `0x80802a67` (size 0x8f0 -> 0x900): same fields at new offsets
- `0x808045d9` -> `0x808045d9` (size 0x320 -> 0x3c0): same fields at new offsets
- `0x80801803` -> `0x80801803` (size 0x5498 -> 0x7050): same fields; nested types changed: `0x808018c7`
- `0x8080265c` -> `0x8080265c` (size 0x4 -> 0x8): `i32` -> `i32x2`
- `0x80801830` -> `0x80801830` (size 0x1b48 -> 0x28c8): same fields; nested types changed: `0x80804b6c`
- `0x80801936` -> `0x80801936` (size 0x444 -> 0x614): same fields at new offsets
- `0x808027e7` -> `0x808027e7` (size 0x100 -> 0x120): same fields at new offsets
- `0x80802145` -> `0x80802145` (size 0x34 -> 0x38): `f32x3` -> `f32x3 i32`
- `0x80801941` -> `0x80801941` (size 0x2d8 -> 0x3b0): `u64?x2 {0x808045f0}? {0x80804681}? {0x80804ee0}? {0x808047ad}? u32?` -> `u64?x2 {0x808045f0}? {0x80804681}? {0x80804ee0}? {0x808010c6}? u32? {0x808010c7}?`
- `0x808017d6` -> `0x808017d6` (size 0x30 -> 0x30): same fields; nested types changed: `0x80801705`, `0x808017f0`, `0x80801a29`
- `0x80801743` -> `0x80801743` (size 0x6ae8 -> 0x6db8): same fields; nested types changed: `0x80801714`
- `0x808023ba` -> `0x808023ba` (size 0x7c -> 0x80): `bool {0x80805c6c} {0x80805c2f}` -> `bool {0x80805c6c} {0x80805c2f} u32`
- `0x808017fd` -> `0x808017fd` (size 0x868 -> 0x908): same fields at new offsets
- `0x8080261d` -> `0x8080261d` (size 0x1310 -> 0x1390): same fields at new offsets
- `0x80800595` -> `0x80800595` (size 0x60 -> 0x70): `{0x8080056f} {0x80800d28} u8:2 u8:3 u32x5 bool f32` -> `{0x8080056f} {0x80800d28} u8:2 u8:3 u32x5 bool f32 i32x4`
- `0x808016dd` -> `0x808016dd` (size 0x440 -> 0x440): same fields; nested types changed: `0x80801716`
- `0x80800f7d` -> `0x80800f7d` (size 0x18 -> 0x24): `u32x6` -> `u32x6 {0x80804060}`
- `0x808000c2` -> `0x808000c2` (size 0xc -> 0xc): `i32 k17:0 k0:0` -> `i32 k17:0 i32`
- `0x808018f8` -> `0x808018f8` (size 0x8 -> 0xc): `{0x80801796}` -> `{0x80801796} bool`
- `0x8080183c` -> `0x8080183c` (size 0x28 -> 0x30): `u64? i16?x3 {0x808046bc}?` -> `u64? i16?x3 {0x80801069}? {0x808046bc}?`
- `0x80800d06` -> `0x80800d06` (size 0xb1c0 -> 0xb1d0): same fields; value ranges or array settings changed
- `0x80801825` -> `0x80801825` (size 0x2f0 -> 0x3c8): same fields; nested types changed: `0x80801796`, `0x80801941`
- `0x808017a5` -> `0x808017a5` (size 0x1398 -> 0x1438): same fields at new offsets
- `0x80801e7a` -> `0x80801e7a` (size 0x20 -> 0x18): same fields; nested types changed: `0x80801a21`
- `0x808000c6` -> `0x808000c6` (size 0x40 -> 0x40): `i32x4 u8:4 u32 k33:0x4 u32 k0:0 k33:0x3` -> `i32x4 u8:4 u32 k33:0x4 u32 k0:0 k33:0x4`
- `0x80800a58` -> `0x80800a58` (size 0x1a8 -> 0x1a8): same fields; nested types changed: `0x80800e98`, `0x80804056`
- `0x808018c7` -> `0x808018c7` (size 0x5498 -> 0x7050): `u64? {0x80804621}? u32? u64? {0x808019c6}? {0x80801716}? {0x80804b6c}? {0x808019ff}? {0x8080472b}? {0x80804b0c}? {0x80805bb9}? {0x80801ea6}? {0x808018` -> `u64? {0x80804621}? u32? u64? {0x808019c6}? {0x80801716}? {0x80801087}? {0x808019ff}? {0x8080472b}? {0x80804745}? {0x80804b0c}? {0x80805bb9}? {0x80801e`
- `0x80803580` -> `0x80803580` (size 0x38 -> 0x38): same fields; nested types changed: `0x808017d6`
- `0x80801855` -> `0x80801855` (size 0x828 -> 0x8c8): same fields at new offsets
- `0x80801954` -> `0x80801954` (size 0x8a0 -> 0x940): same fields at new offsets
- `0x808019c9` -> `0x808019c9` (size 0x28 -> 0x20): same fields; nested types changed: `0x80801a21`
- `0x80802310` -> `0x80802310` (size 0xe0 -> 0x120): same fields; value ranges or array settings changed
- `0x80801701` -> `0x80801701` (size 0x1260 -> 0x1430): same fields at new offsets
- `0x80801714` -> `0x80801714` (size 0x6ae8 -> 0x6db8): same fields at new offsets
- `0x808018eb` -> `0x808018eb` (size 0x6af0 -> 0x6dc0): same fields; nested types changed: `0x80801714`
- `0x80802ddb` -> `0x80802ddb` (size 0x70 -> 0x80): `{0x8080056f} u8:3 {0x80800d28}x2` -> `{0x8080056f} u8:3 {0x80800d28}x2 {0x80800193}`
- `0x808017f0` -> `0x808017f0` (size 0x14 -> 0x15): `bool? u8:4? u8:3? u8:4? u8:2?x3 u8:3? u8:2? u8:3? u8:2? u8:3? u8:4?x2 bool?x6` -> `bool? u8:4? u8:3? u8:4? u8:2?x3 u8:3? u8:2? u8:3? u8:2? u8:3? u8:4?x2 bool?x6 u8:3?`
- `0x80801a21` -> `0x80801a21` (size 0x20 -> 0x18): same fields; nested types changed: `0x808059e9`
- `0x80801716` -> `0x80801716` (size 0x440 -> 0x440): same fields; nested types changed: `0x8080172c`, `0x808017d6`
- `0x80802865` -> `0x80802865` (size 0x1b0 -> 0x230): same fields; value ranges or array settings changed
- `0x80801e9f` -> `0x80801e9f` (size 0x14 -> 0x18): same fields; value ranges or array settings changed
- `0x80804572` -> `0x80804572` (size 0x320 -> 0x3c0): same fields; nested types changed: `0x808045d9`
- `0x80801813` -> `0x80801813` (size 0x54a0 -> 0x7058): same fields; nested types changed: `0x808018c7`
- `0x80801827` -> `0x80801827` (size 0x2e0 -> 0x3b8): same fields; nested types changed: `0x80801941`
- `0x80802f50` -> `0x80802f50` (size 0xb1b0 -> 0xb1b8): `bool i32 i16 {0x80804fb5} {0x80802f84}? {0x80805759}? {0x8080579f}? f32? {0x80802d54}? {0x80802d4a}? {0x80802e0c}? {0x80802fae}? {0x80802ea7}? {0x8080` -> `bool i32 i16 {0x80804fb5} u8:4 {0x80802f84}? {0x80805759}? {0x8080579f}? f32? {0x80802d54}? {0x80802d4a}? {0x80802e0c}? {0x80802fae}? {0x80802ea7}? {0`
- `0x8080271d` -> `0x8080271d` (size 0x100 -> 0x140): same fields; nested types changed: `0x80802219`
- `0x80803218` -> `0x80803218` (size 0x18 -> 0x28): `{0x8080055b} i32` -> `{0x8080055b}x2 i32`
- `0x80800e98` -> `0x80800e98` (size 0x100 -> 0x100): `u32 u64 {0x80800c2d} {0x80800b1f} bool u16 u32 u16` -> `u32 u64 {0x80800c2d} {0x80800b1f} bool u32:17 u32 u16`
- `0x80804655` -> `0x80805fb6` (size 0x30 -> 0x20): same fields at new offsets
- `0x808019ca` -> `0x80801054` (size 0x4 -> 0x4): same fields; value ranges or array settings changed
- `0x808047ad` -> `0x80805fae` (size 0x34 -> 0x24): same fields; nested types changed: `0x80804655`
- `0x808000f0` -> `0x80801068` (size 0x4 -> 0x4): same fields; value ranges or array settings changed
- `0x80804687` -> `0x808002d4` (size 0x100 -> 0x100): same fields; nested types changed: `0x80804707`
- `0x808000f4` -> `0x808010ad` (size 0x4 -> 0x4): same fields; value ranges or array settings changed
- `0x808000f1` -> `0x8080109c` (size 0x4 -> 0x4): same fields; value ranges or array settings changed

## Removed types (25)

`0x8080319e`, `0x808028ef`, `0x80804b6c`, `0x8080479b`, `0x80800074`, `0x80802b20`, `0x808050e3`, `0x808059a1`, `0x808059e9`, `0x80804707`, `0x808000f3`, `0x80805ac6`, `0x80800073`, `0x808018d1`, `0x80804728`, `0x80805c21`, `0x80804de9`, `0x808051bf`, `0x8080191c`, `0x808000ef`, `0x808000f2`, `0x80804278`, `0x80805bc3`, `0x80805bcb`, `0x808000f5`

## Added types (124)

`0x80800fae`, `0x80800302`, `0x808028ef`, `0x80800f99`, `0x8080319e`, `0x808018d1`, `0x80805faf`, `0x80805fb0`, `0x80805fb1`, `0x80801035`, `0x80805fb4`, `0x80801086`, `0x8080107f`, `0x8080106b`, `0x80805fb5`, `0x8080032e`, `0x80800307`, `0x808002ff`, `0x80805fb7`, `0x80800074`, `0x8080191c`, `0x80805fb9`, `0x808010c9`, `0x80802b20`, `0x808010c7`, `0x80801eb8`, `0x80801eaf`, `0x80801eb0`, `0x80800f93`, `0x8080030f`, `0x80801034`, `0x80801089`, `0x808010c8`, `0x8080032f`, `0x8080103a`, `0x80801eb3`, `0x80801038`, `0x808010d0`, `0x808010ec`, `0x808010d7`, `0x8080103b`, `0x80800fb0`, `0x808002d6`, `0x80801087`, `0x8080103c`, `0x80800073`, `0x80801095`, `0x80801082`, `0x808010eb`, `0x808010ee`, `0x80805fc2`, `0x80800fb1`, `0x808010c6`, `0x808002d5`, `0x80801eb9`, `0x80800fc5`, `0x80800faf`, `0x808019ca`, `0x808010c2`, `0x80800fb4`, `0x80805fb2`, `0x80801ea8`, `0x8080107e`, `0x8080101d`, `0x80800fb3`, `0x808010e4`, `0x80805fc6`, `0x80805fc7`, `0x80800fa3`, `0x80801028`, `0x808002fd`, `0x80800f94`, `0x808051bf`, `0x80805fb8`, `0x808010ce`, `0x80801eb1`, `0x808010c4`, `0x808010bb`, `0x80800fe9`, `0x8080101f`, `0x80805fd6`, `0x80801088`, `0x80801026`, `0x8080102b`, `0x808010d5`, `0x8080104d`, `0x80801094`, `0x80800fce`, `0x80801025`, `0x80801eae`, `0x80801036`, `0x80801081`, `0x80801069`, `0x80804278`, `0x80800304`, `0x808010d6`, `0x8080102a`, `0x80800fc4`, `0x80801056`, `0x8080102f`, `0x80805c21`, `0x8080102d`, `0x808010cf`, `0x80800f9e`, `0x80800308`, `0x8080106a`, `0x808010b4`, `0x80801eb7`, `0x808010a3`, `0x8080104b`, `0x80805fc1`, `0x8080223d`, `0x80805e61`, `0x80801023`, `0x8080108c`, `0x8080031e`, `0x80801085`, `0x80800f9b`, `0x80805fca`, `0x8080032d`, `0x80801ebb`, `0x80805fdb`, `0x80800f9f`, `0x80801020`

## Strings: 2943 removed, 1759 added

Added (selected):

- `->investment)`
- `08sys_event_queue_create(&jqPrintServiceEventQueue, &attr, SYS_EVENT_QUEUE_LOCAL, 8) failed: 0x%08x
`
- `_activity>`
- `activity host`
- `activity_option_channel_component_system_globals`
- `bootflow:confirm_account_transfer`
- `c:\bungie\external_libraries\DemonWare\44\bdLobby\bdAuthService-ps3.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdLobby\bdAuthService.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdNet\bdGetHostByName\bdGetHostByName-ps3.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdNet\bdGetHostByName\bdGetHostByName.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdNet\bdGetHostByName\bdGetHostByNameConfig.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdPlatform\bdPlatformError\bdPlatformError.cpp`
- `investment_roster`
- `investment_social_presence_manager`
- `network_bap_nat_punch_queue`
- `ui_bootflow_unp2_0`

Removed (selected):

- `13bdAuthService`
- `14bdLobbyService`
- `Activity`
- `Activity Start`
- `BD_AUTH_PS3_SERVICE_ID_MISMATCH`
- `BD_COMMERCE_MMP_SERVICE_ERROR`
- `BD_ENTITLEMENTS_CONFIG_ERROR`
- `BD_ENTITLEMENTS_ERROR`
- `BD_ENTITLEMENTS_INVALID_GET_MODE_FOR_TITLE`
- `BD_ENTITLEMENTS_INVALID_LEADERBOARDID`
- `BD_ENTITLEMENTS_INVALID_TITLEID`
- `BD_ENTITLEMENTS_NAMED_KEY_ERROR`
- `BD_ENTITLEMENTS_NAMED_PARENT_ERROR`
- `BD_ENTITLEMENTS_URL_CONNECTION_ERROR`
- `BD_EXTERNAL_STORAGE_SERVICE_ERROR`
- `BD_MARKETPLACE_MISSING_REQUIRED_ENTITLEMENT`
- `BD_MARKETPLACE_MMP_SERVICE_ERROR`
- `BD_SERVICE_NOT_AVAILABLE`
- `BD_TWITCH_SERVICE_ERROR`
- `BD_UCD_SERVICE_DISABLED`
- `BD_UCD_SERVICE_ERROR`
- `BD_YOUTUBE_SERVICE_COMMUNICATION_ERROR`
- `BD_YOUTUBE_SERVICE_ERROR`
- `Connected to MatchMaking Service.`
- `Disconnected from MatchMaking Service.`
- `Not connected to LSG. The Content Streaming service is therefore inaccessible.`
- `Not connected to LSG. The Pooled Storage service is therefore inaccessible.`
- `Received message of type: BD_LOBBY_SERVICE_PUSH_MESSAGE`
- `Received message of type: BD_LOBBY_SERVICE_TASK_REPLY`
- `Received message of type: BD_LSG_SERVICE_TASK_REPLY`
- `activity_failed_to_load_missing_dlc`
- `activity_failed_to_load_missing_permissions`
- `activity_join_in_progress`
- `activity_recycling`
- `activity_recycling_keep_only_fireteam`
- `activity_recycling_previous_match_was_blowout`
- `activity_rejoin_in_progress`
- `bdRelayService`
- `bdTeams::setRichPresence is deprecated, use bdRichPresenceService::setInfo instead`
- `c:\bungie\external_libraries\DemonWare\43\bdLobby\bdAuthService-ps3.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdLobby\bdAuthService.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdLobby\bdLobbyService.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdLobby\bdPresence\bdPresenceService.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdLobby\bdRelayService\bdRelayService.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdLobby\bdRichPresence\bdRichPresenceService.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdNet\bdGetHostByName\bdGetHostByName-ps3.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdNet\bdGetHostByName\bdGetHostByName.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdNet\bdGetHostByName\bdGetHostByNameConfig.cpp`
- `c:\bungie\external_libraries\DemonWare\43\bdPlatform\bdPlatformError\bdPlatformError.cpp`
- `c_cui_investment_character_state_component`
- `deletePlayersEntitlements`
- `demonware-session-activity-search`
- `error.account_signed_out`
- `error.activity_not_ready_to_spawn`
- `error.activity_world_not_loaded`
- `error.application_inactive`
- `error.application_not_up_to_date`
- `error.booted_from_private_activity_host_ambassador_revocation_ack_timeout`
- `error.booted_from_private_activity_host_connectivity_failure`
- `error.booted_from_private_activity_host_debug`
- `error.booted_from_private_activity_host_disconnect`
- `error.booted_from_private_activity_host_failed_world_server_connectivity`
- `error.booted_from_private_activity_host_failed_world_server_validation`
- `error.booted_from_private_activity_host_inactivity_timeout`
- `error.booted_from_private_activity_host_reservation_failure`
- `error.booted_from_private_activity_host_session_lock_timeout`
- `error.booted_from_private_activity_host_session_split`
- `error.booted_from_private_activity_host_unknown`
- `error.bungie_server_connection`
- `error.cdn_content_error`
- `error.content_manifest_guid_mismatch`
- `error.content_manifest_invalid`
- `error.content_manifest_not_provided`
- `error.content_manifest_signature_failure`
- `error.content_manifest_url_invalid`
- `error.content_ownership_changed`
- `error.demonware_auth_device_id_error`
- `error.demonware_auth_dns_resolution_error`
- `error.demonware_auth_environment_error`
- `error.demonware_auth_generic_error`
- `error.demonware_auth_platform_token_decryption_error`
- `error.demonware_auth_platform_token_expired`
- `error.demonware_auth_platform_token_retrieval_error`
- `error.demonware_auth_platform_token_signature_error`
- `error.demonware_auth_server_unavailable_error`
- `error.demonware_connection_failed_to_initiate`
- `error.demonware_connection_multiple_sign_on`
- `error.demonware_connection_not_connected`
- `error.demonware_connection_timeout`
- `error.demonware_invalid_user_id`
- `error.demonware_service_not_available`
- `error.deployment_drain_terminal`
- `error.deployment_is_draining`
- `error.dirty_disc`
- `error.disposing_managed_session`
- `error.durango_fireteam_party_not_failed`
- `error.entitlement_enumeration_failed`
- `error.eula_rejected`
- `error.fireteam_double_migration`
- `error.fireteam_local_peer_joined_new_party`
- `error.fireteam_party`
- `error.fireteam_rescue`
- `error.fireteam_rescue_failed`
- `error.fireteam_rescue_timed_out`
- `error.fireteam_size_requirment_failed`
- `error.forced_ktl`
- `error.forced_kto`
- `error.group_double_migration`
- `error.illegal_dlc_media`
- `error.inactive_player_accepted_invite`
- `error.inactivity_timed_out`
- `error.inactivity_timed_out_leading_to_ktl`
- `error.insufficient_content_install_space`
- `error.invalid_launch_media`
- `error.investment_account_version`
- `error.investment_peer_manager`
- `error.investment_server_object_family_record_not_found`
- `error.investment_server_object_instance_not_found`
- `error.investment_server_object_instance_type_from_checksum_not_found`
- `error.investment_server_object_queuez_id_mismatch_in_header_and_rsat_payload`
- `error.investment_server_object_queuez_version_out_of_order`
- `error.ip_address_not_resolved`
- `error.loading_activity_resources`
- `error.loading_orbit_resources`
- `error.lost_bap_connection`
- `error.lost_fireteam`
- `error.lost_private_activity_session`
- `error.lost_worst_change_reason_ft_rescue`
- `error.marketplace_content_fail`
- `error.mtu_out_of_range`
- `error.networking_not_ok`
- `error.no_bap_connection`
- `error.no_cdn_connection`
- `error.no_demonware_connection`
- `error.no_ethernet_link`
- `error.no_investment_connection`
- `error.party_no_longer_exists`
- `error.party_too_many_local_members`
- `error.platform_invalid_privileges`
- `error.platform_no_gold_account`
- `error.platform_not_required_age`
- `error.platform_online_services_unavailable`
- `error.platform_online_services_unreachable`
- `error.platform_session_not_failed`
- `error.platform_signed_out`
- `error.platform_using_guest_account`
- `error.posse_double_migration`
- `error.private_activity_host_error_booted`
- `error.private_activity_host_error_client_became_unestablished`
- `error.private_activity_host_error_connecting_to_bap`
- `error.private_activity_host_error_error_initializing_ipahp`
- `error.private_activity_host_error_rejected_host_join`
- `error.private_activity_host_error_sending_host_join_request`
- `error.private_activity_host_error_sending_host_startup_request`
- `error.private_activity_host_error_session_host_migration`
- `error.private_activity_host_error_setting_parameters`
- `error.private_activity_host_error_unknown`
- `error.private_activity_host_failed_to_initialize_messaging_interface`
- `error.private_activity_host_failed_to_resolve_ah_transport_address_creating_client`
- `error.private_activity_host_failed_to_resolve_ahp_transport_address_creating_client`
- `error.private_activity_host_host_crashed`
- `error.private_activity_host_lost_connection_to_bap`
- `error.private_activity_host_lost_session`
- `error.private_activity_host_ok_unknown`
- `error.private_activity_host_rejected_host_join_should_retry`
- `error.private_activity_host_session_parameter_mismatch`
- `error.private_activity_host_timed_out_connecting_to_bap`
- `error.private_activity_host_timed_out_creating_client`
- `error.private_activity_host_timed_out_initializing_ipahp`
- `error.private_activity_host_timed_out_waiting_for_all_session_peers_to_establish`
- `error.private_activity_host_timed_out_waiting_for_entity_indices`
- `error.private_activity_host_timed_out_waiting_for_host_identifier`
- `error.private_activity_host_timed_out_waiting_for_host_startup_response`
- `error.private_activity_host_timed_out_waiting_for_join_response`
- `error.private_activity_host_timed_out_waiting_for_session_stabilization`
- `error.private_activity_host_timed_out_waiting_to_establish_client`
- `error.private_activity_host_timed_out_waiting_to_send_host_join_request`
- `error.private_activity_instance_timed_out_during_hard_wipe`
- `error.private_activity_instance_timed_out_during_teleportation`
- `error.recycling_activity`
- `error.signon_commercialization_error`
- `error.signon_configuration_error`
- `error.signon_denied`
- `error.signon_error`
- `error.signon_internal_server_error`
- `error.signon_platform_error`
- `error.signon_version_error`
- `error.simulation_aborted`
- `error.sos_prelaunch`
- `error.sstm_activity_host_session_parameter_mismatch`
- `error.sstm_booted_from_public_current_activity_host_ambassador_revocation_ack_timeout`
- `error.sstm_booted_from_public_current_activity_host_connectivity_failure`
- `error.sstm_booted_from_public_current_activity_host_debug_command`
- `error.sstm_booted_from_public_current_activity_host_disconnect`
- `error.sstm_booted_from_public_current_activity_host_failed_world_server_validation`
- `error.sstm_booted_from_public_current_activity_host_inactivity_timeout`
- `error.sstm_booted_from_public_current_activity_host_reservation_failure`
- `error.sstm_booted_from_public_current_activity_host_session_lock_timeout`
- `error.sstm_booted_from_public_current_activity_host_session_split`
- `error.sstm_booted_from_public_current_activity_host_unknown`
- `error.sstm_booted_from_public_target_activity_host_ambassador_revocation_ack_timeout`
- `error.sstm_booted_from_public_target_activity_host_connectivity_failure`
- `error.sstm_booted_from_public_target_activity_host_debug_command`
- `error.sstm_booted_from_public_target_activity_host_disconnect`
- `error.sstm_booted_from_public_target_activity_host_failed_world_server_validation`
- `error.sstm_booted_from_public_target_activity_host_inactivity_timeout`
- `error.sstm_booted_from_public_target_activity_host_reservation_failure`
- `error.sstm_booted_from_public_target_activity_host_session_lock_timeout`
- `error.sstm_booted_from_public_target_activity_host_session_split`
- `error.sstm_booted_from_public_target_activity_host_unknown`
- `error.sstm_citizen_join_failed`
- `error.sstm_could_not_start_public_current_activity_instance`
- `error.sstm_could_not_start_public_target_activity_instance`
- `error.sstm_failed_to_load`
- `error.sstm_not_ok_unknown`
- `error.sstm_precaching_failed`
- `error.sstm_public_activity_instance_doesnt_match_ambassador`
- `error.sstm_public_current_activity_host_error_booted`
- `error.sstm_public_current_activity_host_error_client_became_unestablished`
- `error.sstm_public_current_activity_host_error_connecting_to_bap`
- `error.sstm_public_current_activity_host_error_initializing_ipahp`
- `error.sstm_public_current_activity_host_error_rejected_host_join`
- `error.sstm_public_current_activity_host_error_sending_host_join_request`
- `error.sstm_public_current_activity_host_error_sending_host_startup_request`
- `error.sstm_public_current_activity_host_error_session_host_migration`
- `error.sstm_public_current_activity_host_error_setting_parameters`
- `error.sstm_public_current_activity_host_error_unknown`
- `error.sstm_public_current_activity_host_failed_to_initialize_messaging_interface`
- `error.sstm_public_current_activity_host_failed_to_resolve_ah_transport_address_creating_client`
- `error.sstm_public_current_activity_host_failed_to_resolve_ahp_transport_address_creating_client`
- `error.sstm_public_current_activity_host_host_crashed`
- `error.sstm_public_current_activity_host_lost_connection_to_bap`
- `error.sstm_public_current_activity_host_lost_session`
- `error.sstm_public_current_activity_host_rejected_host_join_should_retry`
- `error.sstm_public_current_activity_host_timed_out_connecting_to_bap`
- `error.sstm_public_current_activity_host_timed_out_creating_client`
- `error.sstm_public_current_activity_host_timed_out_initializing_ipahp`
- `error.sstm_public_current_activity_host_timed_out_waiting_for_all_session_peers_to_establish`
- `error.sstm_public_current_activity_host_timed_out_waiting_for_entity_indices`
- `error.sstm_public_current_activity_host_timed_out_waiting_for_host_identifier`
- `error.sstm_public_current_activity_host_timed_out_waiting_for_host_startup_response`
- `error.sstm_public_current_activity_host_timed_out_waiting_for_join_response`
- `error.sstm_public_current_activity_host_timed_out_waiting_for_session_stabilization`
- `error.sstm_public_current_activity_host_timed_out_waiting_to_establish_client`
- `error.sstm_public_current_activity_host_timed_out_waiting_to_send_host_join_request`
- `error.sstm_public_current_activity_instance_ok`
- `error.sstm_public_matchmaking_failed`
- `error.sstm_public_target_activity_host_error_booted`
- `error.sstm_public_target_activity_host_error_client_became_unestablished`
- `error.sstm_public_target_activity_host_error_connecting_to_bap`
- `error.sstm_public_target_activity_host_error_initializing_ipahp`
- `error.sstm_public_target_activity_host_error_rejected_host_join`
- `error.sstm_public_target_activity_host_error_sending_host_join_request`
- `error.sstm_public_target_activity_host_error_sending_host_startup_request`
- `error.sstm_public_target_activity_host_error_session_host_migration`
- `error.sstm_public_target_activity_host_error_setting_parameters`
- `error.sstm_public_target_activity_host_error_unknown`
- `error.sstm_public_target_activity_host_failed_to_initialize_messaging_interface`
- `error.sstm_public_target_activity_host_failed_to_resolve_ah_transport_address_creating_client`
- `error.sstm_public_target_activity_host_failed_to_resolve_ahp_transport_address_creating_client`
- `error.sstm_public_target_activity_host_host_crashed`
- `error.sstm_public_target_activity_host_lost_connection_to_bap`
- `error.sstm_public_target_activity_host_lost_session`
- `error.sstm_public_target_activity_host_rejected_host_join_should_retry`
- `error.sstm_public_target_activity_host_timed_out_connecting_to_bap`
- `error.sstm_public_target_activity_host_timed_out_creating_client`
- `error.sstm_public_target_activity_host_timed_out_initializing_ipahp`
- `error.sstm_public_target_activity_host_timed_out_waiting_for_all_session_peers_to_establish`
- `error.sstm_public_target_activity_host_timed_out_waiting_for_entity_indices`
- `error.sstm_public_target_activity_host_timed_out_waiting_for_host_identifier`
- `error.sstm_public_target_activity_host_timed_out_waiting_for_host_startup_response`
- `error.sstm_public_target_activity_host_timed_out_waiting_for_join_response`
- `error.sstm_public_target_activity_host_timed_out_waiting_for_session_stabilization`
- `error.sstm_public_target_activity_host_timed_out_waiting_to_establish_client`
- `error.sstm_public_target_activity_host_timed_out_waiting_to_send_host_join_request`
- `error.sstm_public_target_activity_instance_ok`
- `error.sstm_slice_failed_to_instantiate`
- `error.sstm_slice_not_found`
- `error.starting_private_activity_instance`
- `error.starting_private_activity_instance_error_booted`
- `error.starting_private_activity_instance_error_client_became_unestablished`
- `error.starting_private_activity_instance_error_connecting_to_bap`
- `error.starting_private_activity_instance_error_error_initializing_ipahp`
- `error.starting_private_activity_instance_error_rejected_host_join`
- `error.starting_private_activity_instance_error_sending_host_join_request`
- `error.starting_private_activity_instance_error_sending_host_startup_request`
- `error.starting_private_activity_instance_error_session_host_migration`
- `error.starting_private_activity_instance_error_setting_parameters`
- `error.starting_private_activity_instance_error_unknown`
- `error.starting_private_activity_instance_failed_to_initialize_messaging_interface`
- `error.starting_private_activity_instance_failed_to_resolve_ah_transport_address_creating_client`
- `error.starting_private_activity_instance_failed_to_resolve_ahp_transport_address_creating_client`
- `error.starting_private_activity_instance_host_crashed`
- `error.starting_private_activity_instance_jip`
- `error.starting_private_activity_instance_lost_connection_to_bap`
- `error.starting_private_activity_instance_lost_session`
- `error.starting_private_activity_instance_rejected_host_join_should_retry`
- `error.starting_private_activity_instance_timed_out_connecting_to_bap`
- `error.starting_private_activity_instance_timed_out_creating_client`
- `error.starting_private_activity_instance_timed_out_initializing_ipahp`
