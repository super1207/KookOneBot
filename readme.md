# KookOnebot

为kook实现[onebot11](https://github.com/botuniverse/onebot-11)协议！

注意，群聊==频道，好友==私信

我们为此项目创建了一个KOOK群，欢迎来玩，邀请链接：https://kook.top/3SEwQj

[红色问答](https://github.com/super1207/redreply)、[MiraiCQ](https://github.com/super1207/MiraiCQ) 与此项目配合更佳，欢迎加入为它们创建的QQ群：920220179、556515826


## 配置文件

config.json 例子： 

```json
{
	"web_port": 8080,
	"web_host": "127.0.0.1",
	"kook_token": "1/MTUyNDY=/snqjxHpGZFdEM50wyZLOpg==",
	"access_token": "123456",
	"reverse_uri": [
				"http://127.0.0.1:55001/OlivOSMsgApi/pp/onebot/default",
                        	"ws://127.0.0.1:5555/some"
			],
	"secret":""
}
```

解释：

web_port：正向http和正向websocket需要这个端口号，若不使用正向http和正向websocket，填0即可。

web_host：正向http和正向websocket需要这个，若想要外网访问，填`0.0.0.0`，若不使用正向http和正向websocket，填`""`即可。

kook_token：kook的token，请到此处去获得：[KOOK 开发者中心 - 机器人 (kookapp.cn)](https://developer.kookapp.cn/app/index)

access_token：正向http、正向websocket、反向websocket需要，若不需要访问密码，填`""`即可。

reverse_uri：反向http和反向websocket需要这个，若不需要反向http或反向ws，填`[]`即可。

secret：反向http需要的HMAC签名，用来验证上报的数据确实来自OneBot，若不需要，填`""`即可。

**注意：所有的字段都是必填的，不可省略！！！**

## 网络协议实现

正向ws

正向http，端口号和正向ws相同，自动识别！

反向ws

反向 http

## API

### 已实现

#### send_group_msg 发送群消息

目前支持文字、图片、at、回复、自定义音乐分享、qq/网易云音乐分享(使用[故梦api](https://blog.gumengya.com/api.html))

#### send_private_msg 发送私聊消息

目前支持文字、图片、回复、自定义音乐分享、qq/网易云音乐分享(使用[故梦api](https://blog.gumengya.com/api.html))

#### get_login_info 获取登录号信息

#### get_stranger_info 获取陌生人信息

年龄为0，性别为unknown

#### get_group_info 获取群信息

成员数和最大成员数暂时为0，待研究。

#### get_group_list 获取群列表

成员数和最大成员数暂时为0，待研究。

#### get_group_member_info 获取群成员信息

成员信息尽力提供，服务器拥有者，被认为是owner，若有加入某角色，被认为是admin，否则被认为是member。

#### get_group_member_list 获取群成员列表

成员信息尽力提供，服务器拥有者，被认为是owner，若有加入某角色，被认为是admin，否则被认为是member。

#### send_msg 发送消息

#### can_send_image 检查是否可以发送图片

直接返回可以

#### get_status 获取运行状态

#### get_version_info 获取版本信息

#### set_group_kick 群组踢人

实际上是踢出服务器

#### delete_msg 撤回消息

#### set_group_leave 退出群组

实际上会退出服务器

#### can_send_record 检查是否可以发送语音

直接返回不可以

#### set_group_name 设置群名

#### set_group_card 设置群名片（群备注）

实际上会设置该用户在服务器中的名字

#### get_friend_list 获取好友列表

实际上获取在bot的私信列表上的人


### 正在研究


get_msg 获取消息(可能需要数据库支持才行)

set_group_add_request 处理加群请求

### 不实现

send_like 发送好友赞(kook没有这个)

get_group_honor_info 获取群荣誉信息(kook没有群荣誉)

get_cookies 获取 Cookies(kook没有这个)

get_csrf_token 获取 CSRF Token(kook没有这个)

get_credentials 获取 QQ 相关接口凭证(kook没有这个)

clean_cache 清理缓存(没必要)

set_restart 重启 OneBot 实现(没必要)

set_group_anonymous 群组匿名(kook没有匿名)

get_forward_msg 获取合并转发消息(kook没有合并转发)

get_image 获取图片(此api已经过时)

set_group_anonymous_ban 群组匿名用户禁言(kook没有匿名)

set_friend_add_request 处理加好友请求(bot不能被加好友)

get_record 获取语音(kook不支持发送语音，所以也不存在获取语音)

set_group_special_title 设置群组专属头衔(kook没有这个)

set_group_ban 群组单人禁言kook的权限机制不好实现这个）

set_group_whole_ban 群组全员禁言kook的权限机制不好实现这个）

set_group_admin 群组设置管理员kook的权限机制不好实现这个）

set_group_add_request 处理加群邀请(kook的bot被邀请就会同意，不需要处理)

## 事件

### 已实现

#### 群消息 

目前接收文字、图片、at、回复

#### 私聊消息

目前接收文字、图片、回复

#### 生命周期

仅connect（反向http没有此事件）。

#### 群成员减少

sub_type只支持leave，无论是被踢还是自己退出，都为leave，operator_id与user_id相同，均为退出的人

bot自己被踢不会触发此事件。

#### 群成员增加

sub_type只支持approve，无论是被邀请还是自己加入，均为approve，operator_id与user_id相同，均为加入的人

#### 群消息撤回

无法获得正确的operator_id，kook没有提供

#### 好友消息撤回

#### 群文件上传

busid 始终为0，也没啥用

#### 心跳

目前固定为5秒一次

### 正在研究

加群请求

### 不实现

好友添加（bot不能被加好友）

群内戳一戳（kook没有这个）

群红包运气王（kook没有这个）

群成员荣誉变更（kook没有这个）

加好友请求（kook没有这个）

群管理员变动（kook的权限机制不好实现这个）

群禁言（kook的权限机制不好实现这个）

加群邀请（bot被邀请就会自己同意）

## 自行编译

注意，通常情况下，如果您不打算参与此项目的开发，就无需自行编译，请直接到release(或者github action)中去下载。

- 安装好[rust编译环境](https://www.rust-lang.org/)。

- 在windows下(powershell)：

```powershell
$ENV:RUSTFLAGS='-C target-feature=+crt-static';cargo run --target=i686-pc-windows-msvc --release
```
- 在linux下(需要先[安装docker](https://docs.docker.com/engine/install/))：
```bash
cargo install cross --git https://github.com/cross-rs/cross
cross build --target i686-unknown-linux-musl --release
```
