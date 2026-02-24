-- hello.lua
-- 基础测试脚本：验证脚本引擎是否正常工作

print("=== MAMU 脚本引擎测试 ===")
print("Hello from Lua!")
print("1 + 2 = " .. tostring(1 + 2))

-- 测试 toast 通知
mamu.toast("脚本执行成功!")

print("=== 测试完成 ===")
