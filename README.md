# qkc-mining-assistant
This project aims at improving mining efficiency for the QuarkChain network by automatically adjusting the miner according to used-, available allowances and current difficulty

## config.json
The `config.json` file needs to be in the same directory as the `qkc-mining-assistant` executable

Example config:

```json
[
    {
        "rpc": "http://jrpc.mainnet.quarkchain.io:38391",
        "miner_dir": ".",
        "miner_exe": "nanominer",
        "fallback_config": {
            "spawn_args": ["fallback_config.ini"],
            "path": "fallback_config.ini",
            "allowances_to_use": null,
            "mine_at_free_allowances_from_max": 0
        },
        "config_files": [
            {
                "spawn_args": ["config1.ini"],
                "path": "config1.ini",
                "mine_at_free_allowances_from_max": 5
            },
            {
                "spawn_args": ["config2.ini"],
                "path": "config2.ini",
                "allowances_to_use": 20,
                "mine_at_free_allowances_from_max": 5
            }
            {
                "spawn_args": ["config_root.ini"],
                "path": "config_root.ini",
                "root_chain": true,
                "mine_at_free_allowances_from_max": 1
            }
        ]
    }
]
```

`miner_dir` - the directory of the `miner_exe`

`miner_exe` - miner executable (e.g. nanominer.exe)

`fallback_config` - config that will be mined if none of the ones in `config_files` is available

`spawn_args` - args passed to the `miner_exe`

`path` - path to the ini config

`root_chain` - optional set to true if the config is intended for root chain

`allowances_to_use` - optional defines the max allowances the miner will use, if not provided it will use the available allowances based on the address' balance and chain id

`mine_at_free_allowances_from_max` - defines how much lower the used allowances may drop from the available allowances (e.g. `mine_at_free_allowances_from_max`: 5, `allowances_to_use`: 15, the program will consider the address ready to mine once used allowances drop to 10)

The fallback will only be mined once all configs that are provided in the `config_files` array have used all their allowances and will immediately be dropped once any of them is available again.
`config_files` are ranked first by root chain, then difficulty, and then position in the array, which means that 2 configs for different addresses on the same chain prioritizes the one which is defined first in the array.

## ini config
```ini
[Ethash]
wallet=0x13d041434910aD2C1893c6A77537B16Cb7b8Ef5b0003c66c
.. additional configuration
```

Essentially the ini, which is loaded from `path`, only needs to contain the Ethash section with the wallet element, any additional configuration the miner might need is up to you

### Donations
QKC `0x13d041434910aD2C1893c6A77537B16Cb7b8Ef5b0000c66c`

ETH `0xBf5C402072c84b8c33fC70D9CC262c232D11be7D`
