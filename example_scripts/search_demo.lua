-- search_demo.lua
-- 搜索演示：展示完整的搜索 API 用法
-- 内存范围使用 UI 默认: Jh,Ch,Ca,Cd,Cb,An

if not mamu.is_process_bound() then
    mamu.toast("请先绑定一个进程!")
    return
end

local RANGES = "Jh,Ch,Ca,Cd,Cb,An"
mamu.toast("PID: " .. tostring(mamu.get_pid()) .. " 开始搜索...")

-- ========== 1. 精确值搜索 ==========
local count = mamu.search("100", "dword", RANGES)
mamu.toast("搜索 dword 100: 找到 " .. tostring(count) .. " 个结果")

-- ========== 2. 改善搜索（筛选） ==========
if count > 0 then
    -- 等待一下让值变化
    mamu.sleep(1000)
    -- 用新值改善搜索
    count = mamu.refine("100", "dword")
    mamu.toast("改善搜索: 剩余 " .. tostring(count) .. " 个结果")
end

-- ========== 3. 读取搜索结果 ==========
if count > 0 then
    local results = mamu.get_results(0, 5)
    for i = 1, #results do
        local r = results[i]
        mamu.toast(string.format("结果 %d: 地址=0x%X 类型=%s", i, r.address, r.type))
    end
end

-- 清除结果准备下一次搜索
mamu.clear_results()

-- ========== 4. 未知值搜索（模糊搜索） ==========
-- count = mamu.fuzzy_search("dword", RANGES)
-- mamu.toast("未知值搜索: 记录 " .. tostring(count) .. " 个地址")
-- mamu.sleep(2000) -- 等待值变化
-- count = mamu.fuzzy_refine("changed")
-- mamu.toast("值已改变: " .. tostring(count) .. " 个")
-- count = mamu.fuzzy_refine("increased")
-- mamu.toast("值增大: " .. tostring(count) .. " 个")

-- ========== 5. 特征码搜索 ==========
-- count = mamu.pattern_search("1A 2B ?? 4D", "Ca,Cd")
-- mamu.toast("特征码搜索: " .. tostring(count) .. " 个结果")

mamu.toast("搜索演示完成!")
