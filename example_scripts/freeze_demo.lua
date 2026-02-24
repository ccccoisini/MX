-- freeze_demo.lua
-- 冻结操作演示脚本（需要先绑定进程且有搜索结果）

print("=== 冻结操作演示 ===")

if not mamu.is_process_bound() then
    mamu.toast("请先绑定一个进程!")
    return
end

local count = mamu.get_result_count()
print("搜索结果数量: " .. tostring(count))

if count == 0 then
    mamu.toast("没有搜索结果，请先搜索!")
    return
end

-- 获取冻结数量
local frozenCount = mamu.get_frozen_count()
print("已冻结数量: " .. tostring(frozenCount))

-- 将第一个搜索结果冻结为 999
local results = mamu.get_results(0, 1)
if results and #results > 0 then
    local addr = results[1].address
    print("冻结地址: 0x" .. string.format("%X", addr) .. " 值: 999")
    mamu.add_frozen(addr, 999, 2) -- type 2 = int32
    mamu.toast("已冻结地址 0x" .. string.format("%X", addr))
end

print("=== 冻结演示完成 ===")
