-- memory_demo.lua
-- 内存操作演示脚本（需要先绑定进程）

print("=== 内存操作演示 ===")

-- 检查驱动是否加载
if not mamu.is_driver_loaded() then
    mamu.toast("驱动未加载!")
    return
end

-- 检查是否绑定了进程
if not mamu.is_process_bound() then
    mamu.toast("请先绑定一个进程!")
    return
end

-- 获取当前绑定的进程 PID
local pid = mamu.get_pid()
print("当前绑定进程 PID: " .. tostring(pid))

-- 获取搜索结果数量
local count = mamu.get_result_count()
print("当前搜索结果数量: " .. tostring(count))

if count > 0 then
    -- 读取第一个搜索结果的地址
    local results = mamu.get_results(0, 1)
    if results and #results > 0 then
        local addr = results[1].address
        local vtype = results[1].type
        print("第一个结果地址: 0x" .. string.format("%X", addr))
        print("值类型: " .. tostring(vtype))

        -- 读取该地址的值 (以 int32 为例)
        local value = mamu.read_int(addr)
        print("当前值: " .. tostring(value))
    end
end

mamu.toast("演示脚本执行完成")
print("=== 演示完成 ===")
