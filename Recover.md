# Recovery

> MongoDB数据库被Hack，所有历史数据丢失！现在需要扫链重建数据。

```sh
db.getCollection("Refer").insertMany(
[
{
    "lower" : "0xacd57a4063797a2c7866bf5f3e553ac2af7c7dac",
    "upper" : "0x7a89d31e45fca8bdfb7a79627f6ac160129919d2",
    "timestamp" : 1743459187
}
])
```

```sh
db.getCollection("User").insertMany(
[
{
    "address" : "0xacd57a4063797a2c7866bf5f3e553ac2af7c7dac",
    "amount" : "778777242852",
    "price" : "0.00000000024370726827",
    "timestamp" : 1743459187
}
])
```

```sh
db.getCollection("Reward").insertMany(
[
{
    "is_rewarded" : false,
    "user_address" : "0xacd57a4063797a2c7866bf5f3e553ac2af7c7dac",
    "rewards" : [
        {
            "address" : "0x7a89d31e45fca8bdfb7a79627f6ac160129919d2",
            "amount" : 33228200000.2
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8310020000.8
        }
    ],
    "timestamp" : 1743459187
}
])
```

db.getCollection("Refer").find({"user_address": "0x04b9739347fd6889cf3b251ed4632e5189108128"})


## 数据 Schema


```json title="Reward"
{
    "_id" : ObjectId("67d5383bbeb7ac7c9cef0656"),
    "is_rewarded" : false,
    "user_address" : "0xfcc595d47d16cb99ac0844d0c9dabb3c47aa937d",
    "rewards" : [
        {
            "address" : "0x2df705c14b99b5a579511b805baa19701225e4e0",
            "amount" : 37437857632.593765
        }
    ],
    "timestamp" : NumberLong(1742026811)
}
```


备份：
```json title="Refer"
{
    "_id" : ObjectId("67d79ae373c08d9fb9dfc706"),
    "lower" : "0x5127239a23f0fee51d30e0703da3fc9c0143ca7f",
    "upper" : "0x932879ce79db810c23441a32b0a82bdad09a2c45",
    "timestamp" : NumberLong(1742183139)
}
{
    "_id" : ObjectId("67d7a98073c08d9fb9dfc707"),
    "lower" : "0x064fa10ba545125736d2c9415bdaca98a94aff02",
    "upper" : "0x773e712ca4146aa3d4cab152ccbd90238822fcde",
    "timestamp" : NumberLong(1742186880)
}
{
    "_id" : ObjectId("67d7ac4d73c08d9fb9dfc70a"),
    "lower" : "0x7e8ef35bcea94cd740d5e680669c2125293e51b8",
    "upper" : "0x136b01fa086e82b500839c61e4fb7794bb5e1142",
    "timestamp" : NumberLong(1742187597)
}
{
    "_id" : ObjectId("67d834f373c08d9fb9dfc70d"),
    "lower" : "0x63537e883e13051d03f16062d3ad2aabb10acdd3",
    "upper" : "0xe56d1c2bbff396c53aa0f65d413683faec4028b8",
    "timestamp" : NumberLong(1742222579)
}
{
    "_id" : ObjectId("67d8361f73c08d9fb9dfc710"),
    "lower" : "0x70a903aa0ce8480599c949d1111d724a2a7671d0",
    "upper" : "0x63537e883e13051d03f16062d3ad2aabb10acdd3",
    "timestamp" : NumberLong(1742222879)
}
{
    "_id" : ObjectId("67d8371573c08d9fb9dfc713"),
    "lower" : "0x45fcbbc2519f53d44d90b92f39d604fbcc64452e",
    "upper" : "0x70a903aa0ce8480599c949d1111d724a2a7671d0",
    "timestamp" : NumberLong(1742223125)
}
{
    "_id" : ObjectId("67d8bb1873c08d9fb9dfc716"),
    "lower" : "0xfe5226e7776faaf651e0b149b4a5a1ff0be032a0",
    "upper" : "0x45fcbbc2519f53d44d90b92f39d604fbcc64452e",
    "timestamp" : NumberLong(1742256920)
}
{
    "_id" : ObjectId("67d8c0a673c08d9fb9dfc719"),
    "lower" : "0x2981cc5478d5df58595f10e53c9b26430cf749f9",
    "upper" : "0x74420d5e525e0df2d60ff6a2fd3e8eba1d5a3eaf",
    "timestamp" : NumberLong(1742258342)
}
{
    "_id" : ObjectId("67d8c13073c08d9fb9dfc71a"),
    "lower" : "0x8a29ff4c7464ff8d3370247db596fbd0351b23f3",
    "upper" : "0x2981cc5478d5df58595f10e53c9b26430cf749f9",
    "timestamp" : NumberLong(1742258480)
}

```

```json title="User"
{
    "_id" : ObjectId("67d7ab4a73c08d9fb9dfc708"),
    "address" : "0x064fa10ba545125736d2c9415bdaca98a94aff02",
    "amount" : "512084992090",
    "price" : "0.00000000024135875003",
    "timestamp" : NumberLong(1742187338)
}
{
    "_id" : ObjectId("67d7ad3073c08d9fb9dfc70b"),
    "address" : "0x7e8ef35bcea94cd740d5e680669c2125293e51b8",
    "amount" : "805295647035",
    "price" : "0.00000000024030542623",
    "timestamp" : NumberLong(1742187824)
}
{
    "_id" : ObjectId("67d835eb73c08d9fb9dfc70e"),
    "address" : "0x63537e883e13051d03f16062d3ad2aabb10acdd3",
    "amount" : "443988852509",
    "price" : "0.00000000022964319564",
    "timestamp" : NumberLong(1742222827)
}
{
    "_id" : ObjectId("67d836c873c08d9fb9dfc711"),
    "address" : "0x70a903aa0ce8480599c949d1111d724a2a7671d0",
    "amount" : "434775754558",
    "price" : "0.00000000023453438662",
    "timestamp" : NumberLong(1742223048)
}
{
    "_id" : ObjectId("67d8377373c08d9fb9dfc714"),
    "address" : "0x45fcbbc2519f53d44d90b92f39d604fbcc64452e",
    "amount" : "429756819774",
    "price" : "0.00000000023686103819",
    "timestamp" : NumberLong(1742223219)
}
{
    "_id" : ObjectId("67d8bbf573c08d9fb9dfc717"),
    "address" : "0xfe5226e7776faaf651e0b149b4a5a1ff0be032a0",
    "amount" : "430001607377",
    "price" : "0.00000000024047753010",
    "timestamp" : NumberLong(1742257141)
}
{
    "_id" : ObjectId("67d8c25873c08d9fb9dfc71b"),
    "address" : "0x2981cc5478d5df58595f10e53c9b26430cf749f9",
    "amount" : "429775373138",
    "price" : "0.00000000024071795930",
    "timestamp" : NumberLong(1742258776)
}
{
    "_id" : ObjectId("67d8c29173c08d9fb9dfc71d"),
    "address" : "0x8a29ff4c7464ff8d3370247db596fbd0351b23f3",
    "amount" : "430394482060",
    "price" : "0.00000000024025043777",
    "timestamp" : NumberLong(1742258833)
}

```


```json title="Reward"
{
    "_id" : ObjectId("67d7ab4a73c08d9fb9dfc709"),
    "is_rewarded" : true,
    "user_address" : "0x064fa10ba545125736d2c9415bdaca98a94aff02",
    "rewards" : [
        {
            "address" : "0x773e712ca4146aa3d4cab152ccbd90238822fcde",
            "amount" : 33145680440.44925
        }
    ],
    "timestamp" : NumberLong(1742187338)
}
{
    "_id" : ObjectId("67d7ad3073c08d9fb9dfc70c"),
    "is_rewarded" : true,
    "user_address" : "0x7e8ef35bcea94cd740d5e680669c2125293e51b8",
    "rewards" : [
        {
            "address" : "0x136b01fa086e82b500839c61e4fb7794bb5e1142",
            "amount" : 33290966939.83033
        }
    ],
    "timestamp" : NumberLong(1742187824)
}
{
    "_id" : ObjectId("67d835eb73c08d9fb9dfc70f"),
    "is_rewarded" : true,
    "user_address" : "0x63537e883e13051d03f16062d3ad2aabb10acdd3",
    "rewards" : [
        {
            "address" : "0xe56d1c2bbff396c53aa0f65d413683faec4028b8",
            "amount" : 34836651604.70312
        }
    ],
    "timestamp" : NumberLong(1742222827)
}
{
    "_id" : ObjectId("67d836c873c08d9fb9dfc712"),
    "is_rewarded" : true,
    "user_address" : "0x70a903aa0ce8480599c949d1111d724a2a7671d0",
    "rewards" : [
        {
            "address" : "0x63537e883e13051d03f16062d3ad2aabb10acdd3",
            "amount" : 34110136748.56691
        },
        {
            "address" : "0xe56d1c2bbff396c53aa0f65d413683faec4028b8",
            "amount" : 8527534187.141727
        }
    ],
    "timestamp" : NumberLong(1742223048)
}
{
    "_id" : ObjectId("67d8377373c08d9fb9dfc715"),
    "is_rewarded" : true,
    "user_address" : "0x45fcbbc2519f53d44d90b92f39d604fbcc64452e",
    "rewards" : [
        {
            "address" : "0x70a903aa0ce8480599c949d1111d724a2a7671d0",
            "amount" : 33775077831.52092
        },
        {
            "address" : "0x63537e883e13051d03f16062d3ad2aabb10acdd3",
            "amount" : 8443769457.88023
        }
    ],
    "timestamp" : NumberLong(1742223219)
}
{
    "_id" : ObjectId("67d8bbf573c08d9fb9dfc718"),
    "is_rewarded" : false,
    "user_address" : "0xfe5226e7776faaf651e0b149b4a5a1ff0be032a0",
    "rewards" : [
        {
            "address" : "0x45fcbbc2519f53d44d90b92f39d604fbcc64452e",
            "amount" : 33267141410.585926
        },
        {
            "address" : "0x70a903aa0ce8480599c949d1111d724a2a7671d0",
            "amount" : 8316785352.6464815
        }
    ],
    "timestamp" : NumberLong(1742257141)
}
{
    "_id" : ObjectId("67d8c25873c08d9fb9dfc71c"),
    "is_rewarded" : false,
    "user_address" : "0x2981cc5478d5df58595f10e53c9b26430cf749f9",
    "rewards" : [
        {
            "address" : "0x74420d5e525e0df2d60ff6a2fd3e8eba1d5a3eaf",
            "amount" : 33233914176.609665
        }
    ],
    "timestamp" : NumberLong(1742258776)
}
{
    "_id" : ObjectId("67d8c29173c08d9fb9dfc71e"),
    "is_rewarded" : false,
    "user_address" : "0x8a29ff4c7464ff8d3370247db596fbd0351b23f3",
    "rewards" : [
        {
            "address" : "0x2981cc5478d5df58595f10e53c9b26430cf749f9",
            "amount" : 33298586567.46196
        },
        {
            "address" : "0x74420d5e525e0df2d60ff6a2fd3e8eba1d5a3eaf",
            "amount" : 8324646641.86549
        }
    ],
    "timestamp" : NumberLong(1742258833)
}

--------

{
    "_id" : ObjectId("67d9222e68c8ef0360ad315f"),
    "is_rewarded" : false,
    "user_address" : "0x0da89c9091272adc0f50b7857b5e475ad0060cc6",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 33804544816.164932
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8451136204.041233
        }
    ],
    "timestamp" : NumberLong(1742283310)
}
{
    "_id" : ObjectId("67d922e568c8ef0360ad3161"),
    "is_rewarded" : false,
    "user_address" : "0xf2b02bf038af0aac27ae3c07e1581ca64712b5af",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34076177579.83397
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8519044394.958492
        }
    ],
    "timestamp" : NumberLong(1742283493)
}
{
    "_id" : ObjectId("67d922ee68c8ef0360ad3163"),
    "is_rewarded" : false,
    "user_address" : "0x00cc28eb296c7c7d1978f4e0908a6184501d41b9",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 33966866273.968807
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8491716568.492202
        }
    ],
    "timestamp" : NumberLong(1742283502)
}
{
    "_id" : ObjectId("67d9241468c8ef0360ad3165"),
    "is_rewarded" : false,
    "user_address" : "0x035e5891dbb471f3827510b2232a466c7db9ccf6",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34045047587.23752
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8511261896.80938
        }
    ],
    "timestamp" : NumberLong(1742283796)
}
{
    "_id" : ObjectId("67d924b368c8ef0360ad3167"),
    "is_rewarded" : false,
    "user_address" : "0x34bdeca348b608354ed48804152808bead91caec",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 33916941813.907642
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8479235453.476911
        }
    ],
    "timestamp" : NumberLong(1742283955)
}
{
    "_id" : ObjectId("67d924dd68c8ef0360ad3169"),
    "is_rewarded" : false,
    "user_address" : "0x0eb92434a442cf36ed745f113f46326fb04b5251",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 33816297211.851406
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8454074302.962852
        }
    ],
    "timestamp" : NumberLong(1742283997)
}
{
    "_id" : ObjectId("67d92a0b68c8ef0360ad316b"),
    "is_rewarded" : false,
    "user_address" : "0xbbe98bb5a6e8d09bf5b0e25d0507f2df3de81392",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34098636255.202034
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8524659063.8005085
        }
    ],
    "timestamp" : NumberLong(1742285323)
}
{
    "_id" : ObjectId("67d92a2c68c8ef0360ad316d"),
    "is_rewarded" : false,
    "user_address" : "0x2a8976945b3edf2565b53502f901b098a46092a3",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34076698350.76388
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8519174587.69097
        }
    ],
    "timestamp" : NumberLong(1742285356)
}
{
    "_id" : ObjectId("67d92a3868c8ef0360ad316f"),
    "is_rewarded" : false,
    "user_address" : "0x52df5040788d85af5cbc71ad7e2446184f40bf60",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 33991970183.712708
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8497992545.928177
        }
    ],
    "timestamp" : NumberLong(1742285368)
}
{
    "_id" : ObjectId("67d92a5468c8ef0360ad3171"),
    "is_rewarded" : false,
    "user_address" : "0x220aacee71836070e023d68598f97f6ad4973986",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34167023903.49739
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8541755975.874348
        }
    ],
    "timestamp" : NumberLong(1742285396)
}
{
    "_id" : ObjectId("67d92a6368c8ef0360ad3173"),
    "is_rewarded" : false,
    "user_address" : "0x1234a3aa10b57c1b1162e3db6a9329a92e5b2014",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 33970643035.80746
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8492660758.951865
        }
    ],
    "timestamp" : NumberLong(1742285411)
}
{
    "_id" : ObjectId("67d92a8968c8ef0360ad3175"),
    "is_rewarded" : false,
    "user_address" : "0xcd4ef48695839a9099b29b3ad80c264aa7439443",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34197709439.464462
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8549427359.866116
        }
    ],
    "timestamp" : NumberLong(1742285449)
}
{
    "_id" : ObjectId("67d92a8c68c8ef0360ad3177"),
    "is_rewarded" : false,
    "user_address" : "0x1c97547ee1b219992009672f11e14633b356b6d2",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34122902350.785213
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8530725587.696303
        }
    ],
    "timestamp" : NumberLong(1742285452)
}
{
    "_id" : ObjectId("67d92ab368c8ef0360ad3179"),
    "is_rewarded" : false,
    "user_address" : "0x53cc56858cba22e30fb49d57f0d6e0c3899d0d7a",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34113401088.882507
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8528350272.220627
        }
    ],
    "timestamp" : NumberLong(1742285491)
}
{
    "_id" : ObjectId("67d92add68c8ef0360ad317b"),
    "is_rewarded" : false,
    "user_address" : "0xb95c1d8d421a1fa6a75aa68a00664eadf064a936",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34065898026.28967
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8516474506.572417
        }
    ],
    "timestamp" : NumberLong(1742285533)
}
{
    "_id" : ObjectId("67d92b0568c8ef0360ad317d"),
    "is_rewarded" : false,
    "user_address" : "0x23fbec90fe7993476acf1a18bf668a9905540b99",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 34003295918.44934
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8500823979.612335
        }
    ],
    "timestamp" : NumberLong(1742285573)
}
{
    "_id" : ObjectId("67d92b1968c8ef0360ad317f"),
    "is_rewarded" : false,
    "user_address" : "0xd457de984ea4ba09b31ff030f092771667bea39d",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 33948289342.728413
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8487072335.682103
        }
    ],
    "timestamp" : NumberLong(1742285593)
}
{
    "_id" : ObjectId("67d92b8968c8ef0360ad3181"),
    "is_rewarded" : false,
    "user_address" : "0x10bfc201e271172c7adf892a592594523cf0860b",
    "rewards" : [
        {
            "address" : "0xbd9f47a514ffee70496c98c56e91468bac128327",
            "amount" : 33759265550.20185
        },
        {
            "address" : "0x4c6c0251a4192d403ea14fe3679d8017c240982e",
            "amount" : 8439816387.550463
        }
    ],
    "timestamp" : NumberLong(1742285705)
}
