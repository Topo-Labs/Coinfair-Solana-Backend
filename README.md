
1. 从server 同步核心crates包
```sh
./script/sync_from_server.sh .aws/hope.pem ubuntu@ec2-43-206-90-117.ap-northeast-1.compute.amazonaws.com:/home/ubuntu/hope_new/crates /home/stevekeol/Code/BlockChain-Projects/Aptos/PE_Labs/Hope-ReferringReward
```

2. 新增修改

```sh
2.1 接入了utoipa，完成了swagger 文档的配置
2.2 controller 新增了get_balance接口，可以查询用户余额
2.3 controller 新增了 /quote 接口，用于查询swap报价，但是目前价格查询还是有问题

```