# Build difference: 01.00 -> 01.24

- Types: 4387 in 01.00, 351 in 01.24; 55 structurally identical, 14 changed, 4318 removed, 296 added.
- Handles renumbered: 28 of 69 matched types.

## Handles the stand-in uses

| 01.00 | 01.24 | match | first use |
| --- | --- | --- | --- |
| `0x808004ac` | `-` | removed | investment.py:16 |
| `0x808004b1` | `-` | removed | bap_server.py:304 |
| `0x808004fa` | `-` | removed | bap_server.py:1157 |
| `0x80800646` | `-` | removed | bap_server.py:359 |
| `0x808008a3` | `-` | removed | bap_server.py:304 |
| `0x808009ed` | `-` | removed | bap_server.py:301 |
| `0x80800a58` | `-` | removed | bap_server.py:227 |
| `0x80800c6f` | `-` | removed | bap_server.py:422 |
| `0x80800cc5` | `-` | removed | bap_server.py:237 |
| `0x80800cf7` | `-` | removed | bap_server.py:302 |
| `0x80800d64` | `-` | removed | bap_server.py:231 |
| `0x80800e76` | `-` | removed | bap_server.py:235 |
| `0x80800e98` | `-` | removed | bap_server.py:225 |
| `0x80800f1a` | `-` | removed | bap_server.py:305 |
| `0x80801743` | `-` | removed | bap_server.py:70 |
| `0x80801774` | `-` | removed | bap_server.py:563 |
| `0x808017fd` | `-` | removed | bap_server.py:76 |
| `0x80801803` | `-` | removed | bap_server.py:67 |
| `0x80801825` | `-` | removed | bap_server.py:515 |
| `0x8080185c` | `-` | removed | bap_server.py:540 |
| `0x80801869` | `-` | removed | bap_server.py:568 |
| `0x80801921` | `-` | removed | bap_server.py:75 |
| `0x80801932` | `-` | removed | investment.py:16 |
| `0x80801941` | `-` | removed | bap_server.py:453 |
| `0x80801951` | `-` | removed | bap_server.py:68 |
| `0x80801954` | `-` | removed | bap_server.py:69 |
| `0x808022a5` | `-` | removed | bap_server.py:304 |
| `0x80802a1b` | `-` | removed | bap_server.py:388 |
| `0x80804008` | `-` | removed | bap_server.py:250 |
| `0x80804056` | `-` | removed | bap_server.py:250 |
| `0x8080409f` | `-` | removed | bap_server.py:237 |
| `0x808043a7` | `-` | removed | bap_server.py:231 |
| `0x80804595` | `-` | removed | bap_server.py:151 |
| `0x808045f0` | `-` | removed | bap_server.py:464 |
| `0x808046e7` | `-` | removed | bap_server.py:540 |
| `0x808047d8` | `-` | removed | bap_server.py:152 |

## Changed types

- `0x808025e0` -> `0x80802141` (size 0x14 -> 0x8): 
- `0x8080083b` -> `0x8080039b` (size 0x14 -> 0x10): 
- `0x80800762` -> `0x8080084a` (size 0x14 -> 0xc): 
- `0x808025f5` -> `0x808028bb` (size 0x14 -> 0xc): 
- `0x80800381` -> `0x80802185` (size 0x14 -> 0x4): 
- `0x808021f2` -> `0x8080476f` (size 0x14 -> 0x8): 
- `0x80800b7b` -> `0x80800367` (size 0x14 -> 0x10): 
- `0x8080268a` -> `0x80801ea5` (size 0x10 -> 0x1): 
- `0x80801ef5` -> `0x8080199a` (size 0x10 -> 0x1): 
- `0x80800965` -> `0x80801eab` (size 0x1 -> 0x1): 
- `0x80802be5` -> `0x80800f3d` (size 0x10 -> 0x1): 
- `0x808026fe` -> `0x808015fb` (size 0x10 -> 0x1): 
- `0x8080073e` -> `0x808015fc` (size 0x10 -> 0x1): 
- `0x80802926` -> `0x808015fe` (size 0x10 -> 0x1): 

## Removed types (4318)

`0x8080051c`, `0x808021e2`, `0x80802c32`, `0x8080278a`, `0x8080286f`, `0x8080209b`, `0x80802bca`, `0x808020fe`, `0x808027ff`, `0x8080079a`, `0x808023ed`, `0x80803454`, `0x80800729`, `0x80800862`, `0x8080219f`, `0x8080257a`, `0x808003a8`, `0x808020a4`, `0x80800e42`, `0x80802652`, `0x80800772`, `0x80802673`, `0x808020bf`, `0x8080091a`, `0x80801b55`, `0x80801ac4`, `0x808029d8`, `0x808005bf`, `0x80802bc0`, `0x808028a5`, `0x80802171`, `0x80802b84`, `0x808027fd`, `0x80802b4a`, `0x80800f08`, `0x80801a4f`, `0x80802528`, `0x80802670`, `0x80802949`, `0x80802924`, `0x8080001d`, `0x808029df`, `0x80802a42`, `0x80802b21`, `0x80802fed`, `0x808022bc`, `0x8080050f`, `0x80802327`, `0x80802304`, `0x808024a3`, `0x80802b08`, `0x80803000`, `0x80801c38`, `0x8080225b`, `0x80802550`, `0x808021f1`, `0x8080278b`, `0x808020ee`, `0x80802858`, `0x8080230d`, `0x80800aa7`, `0x808006cd`, `0x80802349`, `0x8080257b`, `0x808007e5`, `0x808006b2`, `0x808033e2`, `0x8080266f`, `0x808003e4`, `0x80801a44`, `0x80800ae2`, `0x808028fc`, `0x808028de`, `0x80802966`, `0x8080208a`, `0x808024c8`, `0x8080215b`, `0x8080033b`, `0x808029c5`, `0x808008b8`, `0x8080234a`, `0x808029de`, `0x8080284f`, `0x80802589`, `0x808028e1`, `0x80802c01`, `0x808029d6`, `0x808004f3`, `0x808009d7`, `0x80801bed`, `0x80802a50`, `0x8080289e`, `0x808024b1`, `0x808004e9`, `0x80800dbd`, `0x8080235b`, `0x808020df`, `0x808028a7`, `0x80802bc4`, `0x8080050a`, `0x80802ab2`, `0x80800584`, `0x80802bcc`, `0x80802058`, `0x80800d79`, `0x80802940`, `0x8080304c`, `0x80801463`, `0x80802a00`, `0x808029ac`, `0x80800844`, `0x80800339`, `0x80800a65`, `0x80802289`, `0x80802a8b`, `0x80802078`, `0x808005b2`, `0x80801a79`, `0x80802967`, `0x80803003`, `0x8080226f`, `0x80800611`, `0x80802311`, `0x80803044`, `0x808004ad`, `0x808022b5`, `0x8080053f`, `0x8080258d`, `0x808007db`, `0x808028c1`, `0x808028d1`, `0x808021ab`, `0x808029d5`, `0x80802165`, `0x80802023`, `0x80802749`, `0x80802645`, `0x808022fb`, `0x808027e2`, `0x80802499`, `0x808024e6`, `0x8080052b`, `0x80800441`, `0x8080145f`, `0x808007ab`, `0x80802527`, `0x808008ac`, `0x808026a1`, `0x808024f2`, `0x808021cd`, `0x80800439`, `0x80800394`, `0x80802565`, `0x808027b2`, `0x80802bb9`, `0x80802026`, `0x80800656`, `0x808005af`, `0x80802398`, `0x8080274c`, `0x8080041c`, `0x80800009`, `0x80800014`, `0x80802792`, `0x808029fa`, `0x80802549`, `0x80802175`, `0x808028db`, `0x80800090`, `0x808026dd`, `0x80802a87`, `0x8080000c`, `0x8080000b`, `0x80802923`, `0x80802342`, `0x80800006`, `0x8080000a`, `0x80800096`, `0x80801c10`, `0x80802781`, `0x8080202d`, `0x808028a8`, `0x80800979`, `0x80801b1e`, `0x80801b42`, `0x8080271e`, `0x80801c0f`, `0x8080070b`, `0x80800660`, `0x808022cf`, `0x80802a79`, `0x808028ef`, `0x80802b92`, `0x80800013`, `0x80800092`, `0x80802303`, `0x80800016`, `0x80802207`, `0x80800093`, `0x80802a77`, `0x80802483`, `0x80800010`, `0x80802460`, `0x80800094`, `0x808013ba`, `0x808021d7`, `0x808020ad`, `0x808021fc`, `0x808025f0`, `0x8080220a`, `0x8080278d`, `0x8080264d`, `0x808023f3`, `0x80800011`, `0x8080208c`, `0x8080000e`, `0x808008e7`, `0x80802b7b`, `0x80800095`, `0x808029c9`, `0x80801b79`, `0x80800003`, `0x80800012`, `0x80800004`, `0x808014bd`, `0x808006f7`, `0x8080282b`, `0x8080206c`, `0x80802723`, `0x80800342`, `0x8080059f`, `0x8080261b`, `0x80802578`, `0x8080285e`, `0x8080040a`, `0x80800091`, `0x8080249c`, `0x80802832`, `0x80800007`, `0x8080203f`, `0x80802667`, `0x80800017`, `0x8080343d`, `0x80802a43`, `0x80800008`, `0x80800641`, `0x80802555`, `0x80802a67`, `0x80802994`, `0x80800602`, `0x8080247c`, `0x8080236c`, `0x8080246c`, `0x80802106`, `0x8080000f`, `0x80800674`, `0x808005b8`, `0x808021b2`, `0x80802bb0`, `0x8080211e`, `0x80802653`, `0x80800015`, `0x808020b1`, `0x80801efc`, `0x8080284a`, `0x80802b7a`, `0x80802681`, `0x80802b26`, `0x8080271f`, `0x808029dc`, `0x8080000d`, `0x80800005`, `0x808020ca`, `0x80805b5c`, `0x808016d6`, `0x80800034`, `0x8080184b`, `0x80802f9a`, `0x8080172e`, `0x80801720`, `0x80801719`, `0x80800425`, `0x808007ed`, `0x808040d7`, `0x808016f9`, `0x80801825`, `0x80801796`, `0x8080175b`, `0x808016dd`, `0x80801884`, `0x80800110`, `0x80802665`, `0x80802719`, `0x80803012`, `0x808018a4`, `0x808025f7`, `0x80802fc4`, `0x80802d80`, `0x80802bad`, `0x80801b5e`, `0x80805b03`, `0x80800a16`, `0x80800102`, `0x80802bac`, `0x8080084e`, `0x8080140b`, `0x80802c72`, `0x80801960`, `0x80802f4d`, `0x808058af`, `0x80800a7d`, `0x80802aeb`, `0x80805794`, `0x808027b3`, `0x80800d64`, `0x808020c3`, `0x808020e2`, `0x808013ab`, `0x80800f1b`, `0x808017e7`, `0x808006bd`, `0x80800936`, `0x80805811`, `0x80805b59`, `0x808040aa`, `0x80801bdd`, `0x80802341`, `0x8080244d`, `0x808000c9`, `0x808029c1`, `0x80800dc1`, `0x80800387`, `0x8080091c`, `0x80801859`, `0x808023cb`, `0x80800479`, `0x80802c30`, `0x80803011`, `0x8080155a`, `0x808049bb`, `0x80800d58`, `0x80804b47`, `0x80802551`, `0x80805488`, `0x8080300a`, `0x80804c0e`, `0x8080547d`, `0x80804076`, `0x8080046b`, `0x80802b48`, `0x8080533d`, `0x80800512`, `0x8080084d`, `0x808026b5`, `0x80802960`, `0x808042e0`, `0x80805b5b`, `0x808023f2`, `0x8080094e`, `0x8080194d`, `0x808016fb`, `0x808018ab`, `0x8080255d`, `0x808018cf`, `0x80800adc`, `0x8080183d`, `0x80801958`, `0x8080006f`, `0x8080572d`, `0x80804638`, `0x80802c88`, `0x80802b2f`, `0x8080019b`, `0x80802de3`, `0x80804b5c`, `0x808033f2`, `0x808008f2`, `0x8080072c`, `0x808029ec`, `0x80800b8c`, `0x808000bb`, `0x80804097`, `0x808049d5`, `0x8080237d`, `0x8080442c`, `0x8080007c`, `0x80804a58`, `0x8080205b`, `0x8080009b`, `0x80802469`, `0x8080159b`, `0x80802f38`, `0x8080007f`, `0x80802515`, `0x80800126`, `0x80800def`, `0x80802c75`, `0x808017d7`, `0x808005a5`, `0x80802621`

## Added types (296)

`0x80800050`, `0x80800074`, `0x8080cff2`, `0x80802a8a`, `0x8080012e`, `0x8080046e`, `0x80800073`, `0x80805bf7`, `0x8080182c`, `0x80804597`, `0x8080300b`, `0x80801a12`, `0x808027b8`, `0x80804c64`, `0x80804cee`, `0x8080414d`, `0x8080426f`, `0x80805be5`, `0x80804cbd`, `0x80804cf1`, `0x80804891`, `0x80804ab0`, `0x80804555`, `0x80804092`, `0x80804944`, `0x808040c6`, `0x80804e99`, `0x808048eb`, `0x80804e65`, `0x80804a09`, `0x808040f5`, `0x80804d70`, `0x80804499`, `0x808047a8`, `0x808041a0`, `0x80804305`, `0x8080498d`, `0x80804b1a`, `0x808049e7`, `0x80804cbf`, `0x80804dcf`, `0x8080449d`, `0x80804a3f`, `0x80804459`, `0x80800127`, `0x808043e8`, `0x80804e53`, `0x80805b01`, `0x80805225`, `0x80805c05`, `0x80804991`, `0x808048b2`, `0x80804c48`, `0x80804ee7`, `0x808045dc`, `0x80805dc3`, `0x808049c2`, `0x808045ff`, `0x8080507f`, `0x808049da`, `0x80804a30`, `0x80804ba4`, `0x80804295`, `0x80804a90`, `0x808041b7`, `0x808040e9`, `0x80804dad`, `0x808040a0`, `0x8080495e`, `0x808043ef`, `0x80804ce7`, `0x80804e19`, `0x80804f57`, `0x808044c0`, `0x8080494a`, `0x80804cf8`, `0x80804e4c`, `0x808048a1`, `0x8080542e`, `0x8080483e`, `0x80805c34`, `0x80804554`, `0x80804cc0`, `0x80804a53`, `0x80804098`, `0x80804b91`, `0x80804fbf`, `0x808040b0`, `0x80804502`, `0x808044f4`, `0x80804199`, `0x8080438a`, `0x808052b1`, `0x80804b9b`, `0x80804b31`, `0x80804d8e`, `0x8080488f`, `0x80805b4d`, `0x80804f5f`, `0x80805c47`, `0x80804548`, `0x80805b84`, `0x80805c4d`, `0x80804bfe`, `0x8080540c`, `0x80804744`, `0x80804483`, `0x80804bb4`, `0x808052dd`, `0x8080484c`, `0x80804464`, `0x80804525`, `0x8080411f`, `0x808043ea`, `0x8080428a`, `0x80804d8c`, `0x8080433d`, `0x80804353`, `0x80804285`, `0x80804630`, `0x80804365`, `0x80804cfd`, `0x80804aae`, `0x80804e02`, `0x808049be`, `0x80805c61`, `0x80804c2c`, `0x80805c67`, `0x80804408`, `0x80804e52`, `0x80805ade`, `0x80804493`, `0x80804dfd`, `0x808041d3`, `0x808044aa`, `0x8080448e`, `0x80804522`, `0x808042a0`, `0x80804ba7`, `0x8080468b`, `0x80805ae2`, `0x80804d74`, `0x8080003a`, `0x80804f34`, `0x80804397`, `0x80804266`, `0x8080425f`, `0x80805b70`, `0x80805338`, `0x80805c73`, `0x80804497`, `0x80804b29`, `0x8080451a`, `0x80804205`, `0x8080534a`, `0x80805b4b`, `0x80805c7a`, `0x808042b5`, `0x8080414f`, `0x80804ac3`, `0x808043c9`, `0x8080447b`, `0x8080494e`, `0x80804cfe`, `0x80804cdd`, `0x808046f0`, `0x8080519c`, `0x808044f5`, `0x80804ec6`, `0x808000d3`, `0x808055ec`, `0x80804dde`, `0x80804f4f`, `0x80804dc2`, `0x80804ce2`, `0x808044e5`, `0x80804278`, `0x80805b69`, `0x80805ae4`, `0x80804115`, `0x808046ad`, `0x80804379`, `0x80804cf5`, `0x80804e62`, `0x80804d90`, `0x8080412e`, `0x80804082`, `0x80804250`, `0x80804336`, `0x8080539d`, `0x80804b3c`, `0x808042ce`, `0x808042a6`, `0x80804202`, `0x80805113`, `0x8080494b`, `0x80804566`, `0x808057f2`, `0x80804766`, `0x8080456f`, `0x808041fb`, `0x80804c61`, `0x808040ce`, `0x808042fe`, `0x8080429d`, `0x80804d91`, `0x80804b3e`, `0x80804a3a`, `0x80805804`, `0x8080427f`, `0x80804a3e`, `0x80804321`, `0x80804221`, `0x808041ad`, `0x80805810`, `0x80800f9e`, `0x80804ebe`, `0x80804651`, `0x80804b5b`, `0x80804cca`, `0x808041d6`, `0x80804b92`, `0x80804085`, `0x80804277`, `0x808047c7`, `0x8080448a`, `0x80805830`, `0x80804a42`, `0x80805636`, `0x8080425e`, `0x808044c7`, `0x80804af4`, `0x80804aee`, `0x80805b52`, `0x80804ac4`, `0x80804982`, `0x80805fc0`, `0x80804405`, `0x808044c5`, `0x80804466`, `0x80804f20`, `0x808043c3`, `0x80804204`, `0x80804af6`, `0x80804f70`, `0x808047d7`, `0x80805443`, `0x80804302`, `0x80805661`, `0x808041a1`, `0x808046cf`, `0x80804547`, `0x80804233`, `0x808047d6`, `0x80805ce7`, `0x8080440e`, `0x808047bd`, `0x808000d1`, `0x808048a8`, `0x80804a92`, `0x8080407e`, `0x80804b83`, `0x808044bd`, `0x80804754`, `0x80804ce4`, `0x8080566e`, `0x80805469`, `0x80804866`, `0x80804ff9`, `0x80805af1`, `0x80804291`, `0x80804a71`, `0x8080422c`, `0x8080424e`, `0x80804e83`, `0x80804e27`, `0x808044e1`, `0x80800f9b`, `0x8080514a`, `0x8080428b`, `0x80804476`, `0x80804b7d`, `0x80804231`, `0x80804bb8`, `0x80804e6c`, `0x8080485e`, `0x80804c1e`, `0x808054a5`, `0x80804c53`, `0x808042e2`, `0x8080415b`, `0x8080496d`, `0x80805ae5`, `0x80805d16`, `0x808043b3`, `0x80802b20`

## Strings: 2379 removed, 2791 added

Added (selected):

- `->investment)`
- `08sys_event_queue_create(&jqPrintServiceEventQueue, &attr, SYS_EVENT_QUEUE_LOCAL, 8) failed: 0x%08x
`
- `7zInvestment Assets Interface`
- `BAP %12s `
- `DLC Patch Containers`
- `Lcinvestment`
- `_activity>`
- `activity host`
- `activity_option_channel_component_system_globals`
- `activity_quarantine`
- `auth service`
- `bdAuthService`
- `bootflow:confirm_account_transfer`
- `c:\bungie\external_libraries\DemonWare\44\bdLobby\bdAuthService-ps3.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdLobby\bdAuthService.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdNet\bdGetHostByName\bdGetHostByName-ps3.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdNet\bdGetHostByName\bdGetHostByName.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdNet\bdGetHostByName\bdGetHostByNameConfig.cpp`
- `c:\bungie\external_libraries\DemonWare\44\bdPlatform\bdPlatformError\bdPlatformError.cpp`
- `c_cui_investment_activity_quota_component`
- `c_cui_investment_character_loot_state_component`
- `c_cui_investment_loot_pipe_component`
- `c_mock_activity_vendor_list_component`
- `c_mock_investment_state_component`
- `checkHostProbes`
- `cui::c_abort_activity_countdown_component`
- `cui::c_bootflow_emergency_event_handler_component`
- `cui::c_bootflow_intro_component`
- `cui::c_bootflow_screen_bounds_event_handler_component`
- `cui::c_bootflow_server_queue_event_handler_component`
- `cui::c_bootflow_signon_server_actions_component`
- `cui::c_bootflow_start_component`
- `cui::c_bootflow_start_event_handler_component`
- `cui::c_bootflow_user_advance_event_handler_component`
- `cui::c_delay_activity_countdown_component`
- `cui::c_director_activity_node_container_widget`
- `cui::c_investment_equip_inventory_item_component`
- `cui::c_investment_item_lock_event_handler`
- `cui::c_investment_item_widget`
- `cui::c_investment_promotion_entry_event_handler_component`
- `cui::c_investment_tracked_offer_upsell_event_handler_component`
- `cui::c_launch_director_activity_component`
- `dlc:map: failed to add resource '%s' to map for container [%08X]`
- `dlc_installer error 0`
- `dlc_installer error 1`
- `dlc_installer error 2`
- `dlc_installer error 3`
- `dlc_installer error 4`
- `dlc_installer error 5`
- `dlc_installer error 6`
- `dlc_installer error 7`
- `dlc_url 0`
- `dlc_url 1`
- `dlc_url 2`
- `dlc_url 3`
- `dlc_url 4`
- `dlc_url 5`
- `dlc_url 6`
- `dlc_url 7`
- `extractServiceType`
- `getHostByNameConfig`
- `gethostbyname`
- `ghost link globals`
- `investment account`
- `investment character`
- `investment_location_manager_tracker`
- `investment_roster`
- `investment_social_presence_manager`
- `investment_user_message_manager`
- `is-bubble-host`
- `network_bap_nat_punch_queue`
- `starting soft kto (private activity repair)`
- `ui_bootflow_unp2_0`
- `world_controller:state:bootflow:check_media: NPDRM check failed.`

Removed (selected):

- `13bdAuthService`
- `14bdLobbyService`
- `Activity Start`
- `BAP %s `
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
- `Could not find any activity to load.`
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
- `activity_rejoin_in_progress`
- `activity_tier`
- `bdTeams::setRichPresence is deprecated, use bdRichPresenceService::setInfo instead`
- `demonware-session-activity-search`
- `e_activity_tier`
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
- `error.demonware_connection_failed_to_initiate`
- `error.demonware_connection_multiple_sign_on`
- `error.demonware_connection_timeout`
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
- `error.lost_fireteam`
- `error.lost_private_activity_session`
- `error.lost_worst_change_reason_ft_rescue`
- `error.marketplace_content_fail`
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
- `error.starting_private_activity_instance_timed_out_waiting_for_all_session_peers_to_establish`
- `error.starting_private_activity_instance_timed_out_waiting_for_entity_indices`
- `error.starting_private_activity_instance_timed_out_waiting_for_host_identifier`
- `error.starting_private_activity_instance_timed_out_waiting_for_host_startup_response`
- `error.starting_private_activity_instance_timed_out_waiting_for_join_response`
- `error.starting_private_activity_instance_timed_out_waiting_for_session_stabilization`
- `error.starting_private_activity_instance_timed_out_waiting_to_establish_client`
- `error.starting_private_activity_instance_timed_out_waiting_to_send_host_join_request`
- `error.state_timed_out`
- `error.state_timed_out_matchmaking`
- `error.transport_method_invalid`
- `error.ui_package_loader_not_ok`
- `error.upsell_required`
- `error.user_banned`
- `m_hostAddrSize != BD_COMMON_ADDR_SERIALIZED_SIZE, `
- `ninvestment_roster`
- `sys_event_queue_create(&jqPrintServiceEventQueue, &attr, SYS_EVENT_QUEUE_LOCAL, 8) failed: 0x%08x
`
- `ui_bootflow_unpatchable_0`
