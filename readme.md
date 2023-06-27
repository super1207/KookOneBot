# KookOnebot

为kook实现onebot11协议！（仅支持正向ws和正向http）

注意，群聊==频道


## 网络协议实现

正向ws

正向http，端口号和正向ws相同，自动识别！

## API

### 已实现

#### send_group_msg 发送群消息

目前支持文字、图片、at

#### get_login_info 获取登录号信息

#### get_stranger_info 获取陌生人信息

年龄为0，性别为unknown

#### get_group_info 获取群信息

成员数和最大成员数为0，因为kook没有返回这个信息

#### get_group_list 获取群列表

成员数和最大成员数为0，因为kook没有返回这个信息

#### get_group_member_info 获取群成员信息

成员信息尽力提供，服务器拥有者，被认为是owner，若有加入某角色，被认为是admin，否则被认为是member。

#### send_msg 发送消息

目前仅支持发送群聊消息

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


### 正在研究

send_private_msg 发送私聊消息

get_msg 获取消息(获取消息需要数据库支持才行)

set_group_ban 群组单人禁言

set_group_whole_ban 群组全员禁言

set_group_admin 群组设置管理员

get_friend_list 获取好友列表

set_group_add_request 处理加群请求／邀请(kook的bot被邀请就会同意，不需要处理)


### 不实现

send_like 发送好友赞(kook没有这个)

get_group_member_list 获取群成员列表(kook支持几十万人，没法实现这个)

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

## 事件

### 已实现

#### 群消息 

目前接收文字、图片、at

#### 生命周期

仅connect

#### 群成员减少

sub_type只支持leave，无论是被踢还是自己退出，都为leave，operator_id与user_id相同，均为退出的人

bot自己被踢不会触发此事件。

#### 群成员增加

sub_type只支持approve，无论是被邀请还是自己加入，均为approve，operator_id与user_id相同，均为加入的人

#### 群消息撤回

无法获得正确的operator_id，kook没有提供

#### 群文件上传

busid 始终为0，也没啥用

### 正在研究

私聊消息

群管理员变动

群禁言

好友消息撤回

加群请求／邀请

### 不实现

好友添加（bot不能被加好友）

群内戳一戳（kook没有这个）

群红包运气王（kook没有这个）

群成员荣誉变更（kook没有这个）

加好友请求（kook没有这个）

心跳（没必要）