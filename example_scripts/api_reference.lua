--[[
================================================================================
  MAMU 脚本 API 完整参考手册
================================================================================

  所有 API 通过全局表 `mamu` 访问。
  搜索功能调用与 UI 相同的底层搜索引擎，行为完全一致。

  版本: 1.0
  引擎: Rust mlua

================================================================================
  目录
================================================================================

  1. 进程管理
  2. 内存读取
  3. 内存写入
  4. 搜索 - 精确值搜索
  5. 搜索 - 改善搜索（再次搜索）
  6. 搜索 - 未知值搜索（模糊搜索）
  7. 搜索 - 特征码搜索
  8. 搜索结果操作
  9. 冻结（锁定值）
  10. 工具函数
  11. 数据类型参考
  12. 内存范围参考
  13. 完整使用示例

================================================================================
  1. 进程管理
================================================================================

  mamu.is_driver_loaded() -> boolean
    检查内核驱动是否已加载。

  mamu.is_process_bound() -> boolean
    检查是否已绑定目标进程。

  mamu.get_pid() -> integer | nil
    获取当前绑定进程的 PID。未绑定时返回 nil。

================================================================================
  2. 内存读取
================================================================================

  所有读取函数读取失败时返回 nil（不会抛出错误）。

  mamu.read_byte(address) -> integer | nil
    读取 1 字节无符号整数 (0~255)。

  mamu.read_short(address) -> integer | nil
    读取 2 字节有符号整数 (int16)。

  mamu.read_ushort(address) -> integer | nil
    读取 2 字节无符号整数 (uint16)。

  mamu.read_int(address) -> integer | nil
    读取 4 字节有符号整数 (int32)。

  mamu.read_uint(address) -> integer | nil
    读取 4 字节无符号整数 (uint32)。

  mamu.read_long(address) -> integer | nil
    读取 8 字节有符号整数 (int64)。

  mamu.read_float(address) -> number | nil
    读取 4 字节单精度浮点数 (float32)。

  mamu.read_double(address) -> number | nil
    读取 8 字节双精度浮点数 (float64)。

  mamu.read_bytes(address, size) -> string | nil
    读取指定长度的原始字节数据，返回 Lua 二进制字符串。

================================================================================
  3. 内存写入
================================================================================

  所有写入函数返回 boolean: true=成功, false=失败。

  mamu.write_byte(address, value) -> boolean
    写入 1 字节 (uint8)。

  mamu.write_short(address, value) -> boolean
    写入 2 字节 (int16)。

  mamu.write_int(address, value) -> boolean
    写入 4 字节 (int32)。

  mamu.write_long(address, value) -> boolean
    写入 8 字节 (int64)。

  mamu.write_float(address, value) -> boolean
    写入 4 字节浮点数 (float32)。

  mamu.write_double(address, value) -> boolean
    写入 8 字节浮点数 (float64)。

  mamu.write_bytes(address, data_string) -> boolean
    写入原始字节数据（Lua 字符串）。

================================================================================
  4. 搜索 - 精确值搜索（新搜索）
================================================================================

  mamu.search(value, type, ranges) -> integer

    在指定内存范围中搜索值。等同于 UI 的"搜索"按钮。

    参数:
      value   (string)  搜索表达式，支持以下格式:
                        "5"         精确值搜索
                        "100~200"   范围搜索（100 到 200 之间的值）
                        "5;10;15"   联合搜索（同时搜索多个值）
      type    (string)  值类型，见「数据类型参考」
      ranges  (string)  内存范围（逗号分隔），见「内存范围参考」

    返回: (integer) 搜索结果数量

    示例:
      mamu.search("100", "dword", "Jh,Ch,Ca,Cd,Cb,An")
      mamu.search("3.14", "float", "Ca,An")
      mamu.search("100~200", "dword", "Ca")
      mamu.search("5;10;15", "dword", "Jh,Ca")

================================================================================
  5. 搜索 - 改善搜索（再次搜索）
================================================================================

  mamu.refine(value, type) -> integer

    在已有搜索结果中筛选。等同于 UI 的"改善搜索"。
    必须先调用 search() 产生结果。

    参数:
      value  (string)  新的搜索值
      type   (string)  值类型

    返回: (integer) 剩余结果数量

    示例:
      mamu.search("100", "dword", "Ca,Jh,Ch,Cd,Cb,An")
      mamu.sleep(2000)  -- 等待游戏中的值变化
      mamu.refine("105", "dword")  -- 筛选变为 105 的地址

================================================================================
  6. 搜索 - 未知值搜索（模糊搜索）
================================================================================

  mamu.fuzzy_search(type, ranges) -> integer

    未知值初始搜索：记录内存中所有值的快照。
    之后使用 fuzzy_refine() 按变化条件筛选。

    参数:
      type    (string)  值类型 (byte/word/dword/qword/float/double)
      ranges  (string)  内存范围

    返回: (integer) 记录的地址数量

  ──────────────────────────────────────────────────────────────────

  mamu.fuzzy_refine(condition, param1?, param2?) -> integer

    根据值的变化条件筛选模糊搜索结果。

    参数:
      condition (string)  筛选条件:
        "unchanged"          值未改变
        "changed"            值已改变
        "increased"          值增大了（任意量）
        "decreased"          值减小了（任意量）
        "increased_by"       值增加了 param1
        "decreased_by"       值减少了 param1
        "increased_range"    值增加量在 param1~param2 之间
        "decreased_range"    值减少量在 param1~param2 之间
        "increased_percent"  值增加了 param1%
        "decreased_percent"  值减少了 param1%
      param1  (integer, 可选)  条件参数 1
      param2  (integer, 可选)  条件参数 2（仅 range 条件需要）

    返回: (integer) 剩余结果数量

    示例:
      -- 搜索一个不断变化的血量值
      mamu.fuzzy_search("dword", "Ca,Jh,Ch,Cd,Cb,An")
      mamu.sleep(3000)
      mamu.fuzzy_refine("decreased")        -- 血量减少了
      mamu.sleep(3000)
      mamu.fuzzy_refine("unchanged")        -- 血量没变
      mamu.fuzzy_refine("decreased_by", 10) -- 血量减少了正好 10

================================================================================
  7. 搜索 - 特征码搜索
================================================================================

  mamu.pattern_search(pattern, ranges) -> integer

    搜索字节特征码/签名（支持通配符）。

    参数:
      pattern (string)  十六进制特征码，空格分隔:
                        "1A 2B 3C"    精确匹配全部字节
                        "1A ?? 3C"    ?? = 通配任意字节
                        "1A ?B 3C"    ?B = 高半字节通配
                        "1A 2? 3C"    2? = 低半字节通配
      ranges  (string)  内存范围

    返回: (integer) 搜索结果数量

    示例:
      mamu.pattern_search("89 ?4 24 ?? ?? ?? ?? E8", "Ca,Cd")

================================================================================
  8. 搜索结果操作
================================================================================

  mamu.get_result_count() -> integer
    获取当前搜索结果总数。

  mamu.get_results(offset?, count?) -> table
    获取搜索结果列表。
    参数: offset (默认 0), count (默认 100, 最大 10000)
    返回: { {address=整数, type="dword"}, ... }

  mamu.get_result_addresses(offset?, count?) -> table
    获取搜索结果地址列表（仅地址，无类型）。
    参数: offset (默认 0), count (默认 100, 最大 10000)
    返回: { 地址1, 地址2, ... }

  mamu.write_results(value, type) -> integer
    将值批量写入所有搜索结果地址。
    返回: 成功写入的数量。

  mamu.clear_results()
    清除所有搜索结果，同步更新 UI 显示。

  mamu.is_searching() -> boolean
    检查是否有异步搜索正在进行中。

================================================================================
  9. 冻结（锁定值）
================================================================================

  mamu.freeze(address, value, type) -> boolean
    冻结指定地址为固定值（持续写入）。

    参数:
      address (integer)  内存地址
      value   (string)   冻结值
      type    (string)   值类型

    示例: mamu.freeze(0x12345678, "100", "dword")

  mamu.unfreeze(address) -> boolean
    解冻指定地址。返回是否成功移除。

  mamu.unfreeze_all()
    解冻所有已冻结的地址。

  mamu.get_freeze_count() -> integer
    获取当前冻结的地址数量。

  mamu.is_frozen(address) -> boolean
    检查指定地址是否被冻结。

================================================================================
  10. 工具函数
================================================================================

  mamu.sleep(milliseconds)
    暂停脚本执行。支持取消检测（每 50ms 检查一次）。

  mamu.toast(message)
    在屏幕上显示 Toast 提示消息。

  mamu.log(message)
    输出消息到脚本执行窗口的日志面板。

  mamu.hex(number) -> string
    将整数格式化为十六进制字符串。
    示例: mamu.hex(255) -> "0xFF"

  mamu.parse_hex(hex_string) -> integer
    将十六进制字符串解析为整数（支持 "0x" 前缀）。
    示例: mamu.parse_hex("0xFF") -> 255

  mamu.time() -> number
    返回当前时间（Unix 时间戳，浮点秒数）。用于计时。

  mamu.is_cancelled() -> boolean
    检查用户是否请求了取消脚本。用于循环中检测。

  mamu.check_cancel()
    如果脚本被取消则抛出错误，直接终止执行。

  mamu.bytes_to_int(data, offset?, type?) -> integer | nil
    从字节字符串中解析整数。
    type: "byte", "short", "int"(默认), "long"

  mamu.bytes_to_float(data, offset?, type?) -> number | nil
    从字节字符串中解析浮点数。
    type: "float"(默认), "double"

================================================================================
  11. 数据类型参考
================================================================================

  类型字符串      别名                  大小    说明
  ─────────────────────────────────────────────────────────
  "byte"         "i8", "u8"            1B      字节
  "word"         "short", "i16", "u16" 2B      短整数
  "dword"        "int", "i32", "u32"   4B      整数（最常用）
  "qword"        "long", "i64", "u64"  8B      长整数
  "float"        "f32", "f"            4B      单精度浮点
  "double"       "f64"                 8B      双精度浮点
  "auto"         -                     4B      自动检测
  "xor"          -                     4B      异或加密搜索

================================================================================
  12. 内存范围参考
================================================================================

  代码   名称             说明                    推荐
  ─────────────────────────────────────────────────────────
  "Jh"   Java Heap        Java 堆内存              ★
  "Ch"   C++ heap         C++ 堆 ([heap])          ★
  "Ca"   C++ alloc        C++ 分配 (scudo/malloc)  ★ 最常用
  "Cd"   C++ .data        共享库数据段             ★
  "Cb"   C++ .bss         共享库 BSS 段            ★
  "Ps"   PPSSPP           PPSSPP 模拟器内存
  "An"   Anonymous        匿名内存（空名区域）
  "J"    Java             Java 其他区域
  "S"    Stack            主线程栈
  "As"   Ashmem           Android 共享内存
  "V"    Video            GPU/视频设备内存
  "Ts"   Thread stack     线程栈
  "Vx"   VDEX             VDEX 文件
  "O"    Other            其他（较慢）
  "Xa"   Code app         应用代码（危险）
  "Xs"   Code system      系统代码（危险）
  "Dx"   DEX              DEX 代码（危险）
  "Jc"   JIT cache        JIT 代码缓存（危险）
  "Oa"   OAT Code         OAT 代码（危险）
  "B"    Bad              无效/危险区域
  "Xx"   No perm          无权限区域（危险）

  UI 默认范围: "Jh,Ch,Ca,Cd,Cb,Ps,An"
  推荐游戏搜索: "Jh,Ch,Ca,Cd,Cb,An"

================================================================================
  13. 完整使用示例
================================================================================
--]]

-- ===== 示例 1: 基础精确值搜索 + 改善 + 修改 =====
--[[
if not mamu.is_process_bound() then
    mamu.toast("请先绑定进程!")
    return
end

local RANGES = "Jh,Ch,Ca,Cd,Cb,An"

-- 第一次搜索: 当前金币 = 1000
local count = mamu.search("1000", "dword", RANGES)
mamu.toast("找到 " .. count .. " 个结果")

-- 花掉一些金币后，金币变为 950
mamu.sleep(3000)
count = mamu.refine("950", "dword")
mamu.toast("改善后: " .. count .. " 个结果")

-- 修改所有结果为 999999
if count > 0 and count < 100 then
    local written = mamu.write_results("999999", "dword")
    mamu.toast("已修改 " .. written .. " 个地址")
end
--]]

-- ===== 示例 2: 未知值搜索（血量） =====
--[[
local RANGES = "Ca,Jh,Ch,Cd,Cb,An"

-- 初始快照
mamu.fuzzy_search("dword", RANGES)
mamu.toast("已记录所有值，请让血量减少...")

-- 受伤后
mamu.sleep(5000)
local count = mamu.fuzzy_refine("decreased")
mamu.toast("减少的值: " .. count)

-- 等待不受伤
mamu.sleep(3000)
count = mamu.fuzzy_refine("unchanged")
mamu.toast("未变化: " .. count)

-- 再次受伤
mamu.sleep(5000)
count = mamu.fuzzy_refine("decreased")
mamu.toast("再次减少: " .. count)

-- 读取结果
if count < 50 then
    local results = mamu.get_results(0, count)
    for i, r in ipairs(results) do
        local val = mamu.read_int(r.address)
        mamu.log(string.format("[%d] 0x%X = %s", i, r.address, tostring(val)))
    end
end
--]]

-- ===== 示例 3: 特征码搜索 =====
--[[
local count = mamu.pattern_search("89 44 24 ?? E8 ?? ?? ?? ?? 8B", "Ca,Cd")
mamu.toast("找到 " .. count .. " 个特征码匹配")

if count > 0 then
    local addrs = mamu.get_result_addresses(0, 10)
    for i, addr in ipairs(addrs) do
        mamu.log(string.format("匹配 %d: %s", i, mamu.hex(addr)))
    end
end
--]]

-- ===== 示例 4: 搜索 + 冻结 =====
--[[
local RANGES = "Jh,Ch,Ca,Cd,Cb,An"
local count = mamu.search("100", "dword", RANGES)

if count > 0 and count < 20 then
    local addrs = mamu.get_result_addresses(0, count)
    for _, addr in ipairs(addrs) do
        mamu.freeze(addr, "999999", "dword")
    end
    mamu.toast("已冻结 " .. #addrs .. " 个地址为 999999")

    -- 30 秒后解冻
    mamu.sleep(30000)
    mamu.unfreeze_all()
    mamu.toast("已解冻所有地址")
end
--]]

-- ===== 示例 5: 读取内存结构 =====
--[[
local base_addr = 0x12345678
local hp     = mamu.read_int(base_addr + 0x00)
local max_hp = mamu.read_int(base_addr + 0x04)
local mp     = mamu.read_float(base_addr + 0x08)
local name   = mamu.read_bytes(base_addr + 0x10, 32)

if hp then
    mamu.log(string.format("HP: %d/%d  MP: %.1f", hp, max_hp or 0, mp or 0))
    mamu.write_int(base_addr + 0x00, 9999)  -- 修改 HP
end
--]]
