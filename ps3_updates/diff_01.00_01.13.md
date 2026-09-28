# Build difference: 01.00 -> 01.13

- Types: 4387 in 01.00, 352 in 01.13; 70 structurally identical, 10 changed, 4307 removed, 300 added.
- Handles renumbered: 42 of 80 matched types.

## Handles the stand-in uses

| 01.00 | 01.13 | match | first use |
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

- `0x808025e0` -> `0x80802141` (size 0x14 -> 0x4): 
- `0x8080083b` -> `0x8080084a` (size 0x14 -> 0xc): 
- `0x80800762` -> `0x80802e9f` (size 0x14 -> 0xc): 
- `0x808025f5` -> `0x8080231c` (size 0x14 -> 0xc): 
- `0x80800381` -> `0x80800012` (size 0x14 -> 0x4): 
- `0x80800965` -> `0x8080199a` (size 0x1 -> 0x1): 
- `0x808015a2` -> `0x808019d2` (size 0x1 -> 0x1): 
- `0x80801500` -> `0x80800f3d` (size 0x1 -> 0x1): 
- `0x80801569` -> `0x808015fb` (size 0x1 -> 0x1): 
- `0x80800078` -> `0x808015fe` (size 0x1 -> 0x1): 

## Removed types (4307)

`0x8080051c`, `0x808021e2`, `0x80802c32`, `0x8080278a`, `0x8080286f`, `0x8080209b`, `0x80802bca`, `0x808021f2`, `0x80800b7b`, `0x808020fe`, `0x808027ff`, `0x8080079a`, `0x808023ed`, `0x80803454`, `0x80800729`, `0x80800862`, `0x8080219f`, `0x8080257a`, `0x808003a8`, `0x808020a4`, `0x80800e42`, `0x80802652`, `0x80800772`, `0x80802673`, `0x808020bf`, `0x8080091a`, `0x80801b55`, `0x80801ac4`, `0x808029d8`, `0x808005bf`, `0x80802bc0`, `0x808028a5`, `0x80802171`, `0x80802b84`, `0x808027fd`, `0x80802b4a`, `0x80800f08`, `0x80801a4f`, `0x80802528`, `0x80802670`, `0x80802949`, `0x80802924`, `0x8080001d`, `0x808029df`, `0x80802a42`, `0x80802b21`, `0x80802fed`, `0x808022bc`, `0x8080050f`, `0x80802327`, `0x80802304`, `0x808024a3`, `0x80802b08`, `0x80803000`, `0x80801c38`, `0x8080225b`, `0x80802550`, `0x808021f1`, `0x8080278b`, `0x808020ee`, `0x80802858`, `0x8080230d`, `0x80800aa7`, `0x808006cd`, `0x80802349`, `0x8080257b`, `0x808007e5`, `0x808006b2`, `0x808033e2`, `0x8080266f`, `0x808003e4`, `0x80801a44`, `0x80800ae2`, `0x808028fc`, `0x808028de`, `0x80802966`, `0x8080208a`, `0x808024c8`, `0x8080215b`, `0x8080033b`, `0x808029c5`, `0x808008b8`, `0x8080234a`, `0x808029de`, `0x8080284f`, `0x80802589`, `0x808028e1`, `0x80802c01`, `0x808029d6`, `0x808004f3`, `0x808009d7`, `0x80801bed`, `0x80802a50`, `0x8080289e`, `0x808024b1`, `0x808004e9`, `0x80800dbd`, `0x8080235b`, `0x808020df`, `0x808028a7`, `0x80802bc4`, `0x8080050a`, `0x80802ab2`, `0x80800584`, `0x80802bcc`, `0x80802058`, `0x80800d79`, `0x80802940`, `0x8080304c`, `0x80801463`, `0x80802a00`, `0x808029ac`, `0x80800844`, `0x80800339`, `0x80800a65`, `0x80802289`, `0x80802a8b`, `0x80802078`, `0x808005b2`, `0x80801a79`, `0x80802967`, `0x80803003`, `0x8080226f`, `0x80800611`, `0x80802311`, `0x80803044`, `0x808004ad`, `0x808022b5`, `0x8080053f`, `0x8080258d`, `0x808007db`, `0x808028c1`, `0x808028d1`, `0x808021ab`, `0x808029d5`, `0x80802165`, `0x80802023`, `0x80802749`, `0x80802645`, `0x808022fb`, `0x808027e2`, `0x80802499`, `0x808024e6`, `0x8080052b`, `0x80800441`, `0x8080145f`, `0x808007ab`, `0x80802527`, `0x808008ac`, `0x808026a1`, `0x808024f2`, `0x808021cd`, `0x80800439`, `0x80800394`, `0x80802565`, `0x808027b2`, `0x80802bb9`, `0x80802026`, `0x80800656`, `0x808005af`, `0x80802398`, `0x8080274c`, `0x8080041c`, `0x80800009`, `0x80800014`, `0x80802792`, `0x808029fa`, `0x80802549`, `0x80802175`, `0x808028db`, `0x80800090`, `0x808026dd`, `0x80802a87`, `0x8080000c`, `0x8080000b`, `0x80802923`, `0x80802342`, `0x80800006`, `0x8080000a`, `0x80800096`, `0x80801c10`, `0x80802781`, `0x8080202d`, `0x808028a8`, `0x80800979`, `0x80801b1e`, `0x80801b42`, `0x8080271e`, `0x80801c0f`, `0x8080070b`, `0x80800660`, `0x808022cf`, `0x80802a79`, `0x808028ef`, `0x80802b92`, `0x80800013`, `0x80800092`, `0x80802303`, `0x80800016`, `0x80802207`, `0x80800093`, `0x80802a77`, `0x80802483`, `0x80800010`, `0x80802460`, `0x80800094`, `0x808013ba`, `0x808021d7`, `0x808020ad`, `0x808021fc`, `0x808025f0`, `0x8080220a`, `0x8080278d`, `0x8080264d`, `0x808023f3`, `0x80800011`, `0x8080208c`, `0x8080000e`, `0x80802b7b`, `0x80800095`, `0x808029c9`, `0x80800003`, `0x80800012`, `0x80800004`, `0x808014bd`, `0x808006f7`, `0x8080282b`, `0x8080206c`, `0x80802723`, `0x8080059f`, `0x8080261b`, `0x80802578`, `0x8080285e`, `0x8080040a`, `0x80800091`, `0x8080249c`, `0x80802832`, `0x80800007`, `0x8080203f`, `0x80802667`, `0x80800017`, `0x8080343d`, `0x80802a43`, `0x80800008`, `0x80800641`, `0x80802555`, `0x80802a67`, `0x80802994`, `0x80800602`, `0x8080247c`, `0x8080236c`, `0x8080246c`, `0x80802106`, `0x8080000f`, `0x80800674`, `0x808005b8`, `0x808021b2`, `0x80802bb0`, `0x8080211e`, `0x80802653`, `0x80800015`, `0x808020b1`, `0x80801efc`, `0x8080284a`, `0x80802b7a`, `0x80802681`, `0x80802b26`, `0x8080271f`, `0x808029dc`, `0x8080000d`, `0x80800005`, `0x808020ca`, `0x80805b5c`, `0x808016d6`, `0x80800034`, `0x8080184b`, `0x80802f9a`, `0x8080172e`, `0x80801720`, `0x80801719`, `0x80800425`, `0x808007ed`, `0x808040d7`, `0x808016f9`, `0x80801825`, `0x80801796`, `0x8080175b`, `0x808016dd`, `0x80801884`, `0x80800110`, `0x80802665`, `0x80802719`, `0x80803012`, `0x808018a4`, `0x808025f7`, `0x80802fc4`, `0x80802d80`, `0x80802bad`, `0x80801b5e`, `0x80805b03`, `0x80800a16`, `0x80800102`, `0x80802bac`, `0x8080084e`, `0x8080140b`, `0x80802c72`, `0x80801960`, `0x80802f4d`, `0x808058af`, `0x80800a7d`, `0x80802aeb`, `0x80805794`, `0x808027b3`, `0x80800d64`, `0x808020c3`, `0x808020e2`, `0x808013ab`, `0x80800f1b`, `0x808017e7`, `0x808006bd`, `0x80800936`, `0x80805811`, `0x80805b59`, `0x808040aa`, `0x80801bdd`, `0x80802341`, `0x8080244d`, `0x808000c9`, `0x808029c1`, `0x80800dc1`, `0x80800387`, `0x8080091c`, `0x80801859`, `0x808023cb`, `0x80800479`, `0x80802c30`, `0x80803011`, `0x8080155a`, `0x808049bb`, `0x80800d58`, `0x80804b47`, `0x80802551`, `0x80805488`, `0x8080300a`, `0x80804c0e`, `0x8080547d`, `0x80804076`, `0x8080046b`, `0x80802b48`, `0x8080533d`, `0x80800512`, `0x8080084d`, `0x808026b5`, `0x80802960`, `0x808042e0`, `0x80805b5b`, `0x808023f2`, `0x8080094e`, `0x8080194d`, `0x808016fb`, `0x808018ab`, `0x8080255d`, `0x808018cf`, `0x80800adc`, `0x8080183d`, `0x80801958`, `0x8080006f`, `0x8080572d`, `0x80804638`, `0x80802c88`, `0x80802b2f`, `0x8080019b`, `0x80802de3`, `0x80804b5c`, `0x808033f2`, `0x808008f2`, `0x8080072c`, `0x808029ec`, `0x80800b8c`, `0x808000bb`, `0x80804097`, `0x808049d5`, `0x8080237d`, `0x8080442c`, `0x8080007c`, `0x80804a58`, `0x8080205b`, `0x8080009b`, `0x80802469`, `0x8080159b`, `0x80802f38`, `0x8080007f`, `0x80802515`, `0x80800126`, `0x80800def`, `0x80802c75`, `0x808017d7`, `0x808005a5`, `0x80802621`, `0x80800918`

## Added types (300)

`0x80800074`, `0x80801719`, `0x8080cff2`, `0x8080299f`, `0x80804b3b`, `0x80800050`, `0x8080433d`, `0x80800073`, `0x80805d1e`, `0x80804810`, `0x80804597`, `0x8080300b`, `0x80801a12`, `0x80801a24`, `0x808027b8`, `0x80804c64`, `0x80804cee`, `0x8080414d`, `0x8080426f`, `0x80805be5`, `0x80804cbd`, `0x80804cf1`, `0x80804891`, `0x80804ab0`, `0x80804555`, `0x80804092`, `0x80804944`, `0x808040c6`, `0x80804e99`, `0x808048eb`, `0x80804e65`, `0x80804a09`, `0x808040f5`, `0x80804d70`, `0x80804499`, `0x808047a8`, `0x808041a0`, `0x80804305`, `0x8080498d`, `0x80804b1a`, `0x808049e7`, `0x80804105`, `0x80804cbf`, `0x80804dcf`, `0x8080449d`, `0x80804a3f`, `0x80804459`, `0x808043e8`, `0x80804e53`, `0x80805b01`, `0x80805225`, `0x80805c05`, `0x80804991`, `0x808048b2`, `0x80804c48`, `0x80804ee7`, `0x808045dc`, `0x80805dc3`, `0x808049c2`, `0x808045ff`, `0x8080507f`, `0x808049da`, `0x80804a30`, `0x80804ba4`, `0x80804295`, `0x80804a90`, `0x808041b7`, `0x808040e9`, `0x80804dad`, `0x808040a0`, `0x8080495e`, `0x808043ef`, `0x80804ce7`, `0x80804e19`, `0x80804f57`, `0x808044c0`, `0x8080494a`, `0x808000e0`, `0x80804cf8`, `0x80804e4c`, `0x808048a1`, `0x80805b73`, `0x8080483e`, `0x80805c34`, `0x80804554`, `0x80804cc0`, `0x80804a53`, `0x80804098`, `0x80804b91`, `0x80804fbf`, `0x808040b0`, `0x80804502`, `0x808044f4`, `0x80804199`, `0x8080438a`, `0x808052b1`, `0x80804b9b`, `0x80804b31`, `0x80804d8e`, `0x8080488f`, `0x80805b4d`, `0x80804f5f`, `0x80805c47`, `0x80804548`, `0x80805b84`, `0x80804b7b`, `0x80805e61`, `0x80805c4d`, `0x80804bfe`, `0x8080540c`, `0x80804744`, `0x80804483`, `0x80804bb4`, `0x808052dd`, `0x8080484c`, `0x80804464`, `0x80804525`, `0x8080411f`, `0x808043ea`, `0x8080428a`, `0x80804d8c`, `0x80804353`, `0x80804285`, `0x80804630`, `0x80804365`, `0x80804cfd`, `0x80804aae`, `0x80804e02`, `0x808049be`, `0x80804c2c`, `0x80805c67`, `0x80804408`, `0x80804e52`, `0x80805ade`, `0x80804493`, `0x80804dfd`, `0x808041d3`, `0x808044aa`, `0x8080448e`, `0x80804522`, `0x808042a0`, `0x80804ba7`, `0x8080468b`, `0x80805ae2`, `0x80804d74`, `0x8080003a`, `0x80804f34`, `0x80804397`, `0x80804266`, `0x8080425f`, `0x80805b70`, `0x80805338`, `0x80805c73`, `0x80804497`, `0x80804b29`, `0x8080451a`, `0x80804205`, `0x8080534a`, `0x80805b4b`, `0x80805c7a`, `0x808042b5`, `0x8080414f`, `0x80804fb7`, `0x80804ac3`, `0x808043c9`, `0x8080447b`, `0x8080494e`, `0x80804cfe`, `0x80804cdd`, `0x808046f0`, `0x8080519c`, `0x808044f5`, `0x80804ec6`, `0x808000d3`, `0x808055ec`, `0x80804dde`, `0x80804f4f`, `0x80804dc2`, `0x80804ce2`, `0x808044e5`, `0x80805b7b`, `0x80805b69`, `0x80805ae4`, `0x80804115`, `0x808046ad`, `0x80804379`, `0x80804cf5`, `0x80804e62`, `0x80804d90`, `0x8080412e`, `0x80804082`, `0x80804250`, `0x80804336`, `0x8080539d`, `0x80804b3c`, `0x808042ce`, `0x808042a6`, `0x80804202`, `0x80805113`, `0x8080494b`, `0x80804566`, `0x808057f2`, `0x80804766`, `0x8080456f`, `0x808041fb`, `0x80804c61`, `0x808043ca`, `0x808040ce`, `0x808042fe`, `0x8080429d`, `0x80804d91`, `0x80804b3e`, `0x80804a3a`, `0x80805804`, `0x8080427f`, `0x80804a3e`, `0x80804321`, `0x80804221`, `0x808041ad`, `0x80805810`, `0x80804ebe`, `0x80804651`, `0x80804b5b`, `0x80804cca`, `0x808041d6`, `0x80804085`, `0x80804277`, `0x808047c7`, `0x8080448a`, `0x80805830`, `0x80804a42`, `0x80805636`, `0x8080425e`, `0x808044c7`, `0x80804af4`, `0x80804aee`, `0x80804e4a`, `0x80805b52`, `0x80804ac4`, `0x80804982`, `0x80804405`, `0x808044c5`, `0x80804466`, `0x80804f20`, `0x8080511b`, `0x808043c3`, `0x80804204`, `0x80804af6`, `0x80804f70`, `0x808047d7`, `0x80805443`, `0x80804302`, `0x80805661`, `0x808041a1`, `0x808046cf`, `0x80804547`, `0x808047d6`, `0x80805ce7`, `0x8080440e`, `0x808047bd`, `0x808000d1`, `0x808048a8`, `0x80804a92`, `0x8080407e`, `0x80804b83`, `0x808044bd`, `0x80804754`, `0x80804ce4`, `0x8080566e`, `0x80805469`, `0x80804866`, `0x80804bf8`, `0x80804ff9`, `0x80805af1`, `0x80804291`, `0x80804a71`, `0x8080422c`, `0x8080424e`, `0x80804e83`, `0x80804e27`, `0x808044e1`, `0x8080514a`, `0x8080428b`, `0x80804476`, `0x80804b7d`, `0x80804231`, `0x80804bb8`, `0x80804e6c`, `0x8080485e`, `0x80804c1e`, `0x808054a5`, `0x80804c53`, `0x808042e2`, `0x8080415b`, `0x8080496d`, `0x80805ae5`, `0x80805d16`, `0x808043b3`, `0x80804dbe`, `0x80802b20`

## Strings: 1402 removed, 2998 added

Added (selected):

- `7zInvestment Assets Interface`
- `Activity`
- `BAP %12s `
- `DLC Patch Containers`
- `Lcinvestment`
- `activity_quarantine`
- `activity_recycling_keep_only_fireteam`
- `activity_recycling_previous_match_was_blowout`
- `auth service`
- `bdAuthService`
- `bdRelayService`
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
- `c_cui_investment_activity_quota_component`
- `c_cui_investment_character_loot_state_component`
- `c_cui_investment_character_state_component`
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
- `deletePlayersEntitlements`
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
- `error.demonware_auth_device_id_error`
- `error.demonware_auth_dns_resolution_error`
- `error.demonware_auth_environment_error`
- `error.demonware_auth_generic_error`
- `error.demonware_auth_platform_token_decryption_error`
- `error.demonware_auth_platform_token_expired`
- `error.demonware_auth_platform_token_retrieval_error`
- `error.demonware_auth_platform_token_signature_error`
- `error.demonware_auth_server_unavailable_error`
- `error.demonware_connection_not_connected`
- `error.demonware_invalid_user_id`
- `error.demonware_service_not_available`
- `error.fireteam_size_requirment_failed`
- `error.lost_bap_connection`
- `error.mtu_out_of_range`
- `extractServiceType`
- `getEntitlements`
- `getHostAddrAsCommonAddr`
- `getHostByNameConfig`
- `getPlayersEntitlements`
- `getRelayService`
- `gethostbyname`
- `ghost link globals`
- `investment account`
- `investment character`
- `investment_location_manager_tracker`
- `investment_user_message_manager`
- `is-bubble-host`
- `lobby service`
- `putPlayersEntitlements`
- `starting soft kto (private activity repair)`
- `ui_bootflow_unp1_0`
- `world_controller:state:bootflow:check_media: NPDRM check failed.`

Removed (selected):

- `BAP %s `
- `Could not find any activity to load.`
- `activity_tier`
- `e_activity_tier`
- `ui_bootflow_unpatchable_0`
