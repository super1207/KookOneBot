# KookOnebot

为kook实现onebot11协议！（仅支持正向ws和正向http）


## 网络协议实现

### 已实现

正向ws

### 待实现

正向http

## API

### 已实现

send_group_msg 发送群消息(目前只支持文字和图片)

### 可以实现

get_login_info 获取登录号信息

get_stranger_info 获取陌生人信息

get_group_info 获取群信息

get_group_list 获取群列表

get_group_member_info 获取群成员信息

set_group_kick 群组踢人



### 正在研究
send_private_msg 发送私聊消息

send_msg 发送消息

delete_msg 撤回消息

get_msg 获取消息

get_forward_msg 获取合并转发消息

set_group_ban 群组单人禁言

set_group_anonymous_ban 群组匿名用户禁言

set_group_whole_ban 群组全员禁言

set_group_admin 群组设置管理员

set_group_anonymous 群组匿名

set_group_card 设置群名片（群备注）

set_group_name 设置群名

set_group_leave 退出群组

set_group_special_title 设置群组专属头衔

set_friend_add_request 处理加好友请求

set_group_add_request 处理加群请求／邀请

get_friend_list 获取好友列表

get_record 获取语音

get_image 获取图片

can_send_image 检查是否可以发送图片

can_send_record 检查是否可以发送语音

get_status 获取运行状态

get_version_info 获取版本信息

### 不实现

send_like 发送好友赞

get_group_member_list 获取群成员列表

get_group_honor_info 获取群荣誉信息

get_cookies 获取 Cookies

get_csrf_token 获取 CSRF Token

get_credentials 获取 QQ 相关接口凭证

clean_cache 清理缓存

set_restart 重启 OneBot 实现

## 事件

### 已实现

群消息 （目前仅接收文字和图片）

生命周期（仅connect）

### 正在研究

其余全部
