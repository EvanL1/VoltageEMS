#!/usr/bin/env python3
"""
验证comsrv读取的位值是否与模拟器设置的值一致
"""

# 模拟器设置的值
SIMULATOR_VALUES = {
    # 寄存器1 (地址1): 0xA5 = 10100101
    1: {
        "register_value": 0xA5,
        "bits": {
            0: 1,  # bit 0 = 1
            1: 0,  # bit 1 = 0
            2: 1,  # bit 2 = 1
            3: 0,  # bit 3 = 0
            4: 0,  # bit 4 = 0
            5: 1,  # bit 5 = 1
            6: 0,  # bit 6 = 0
            7: 1,  # bit 7 = 1
        },
    },
    # 寄存器2 (地址2): 0x5A = 01011010
    2: {
        "register_value": 0x5A,
        "bits": {
            0: 0,  # bit 0 = 0
            1: 1,  # bit 1 = 1
            2: 0,  # bit 2 = 0
            3: 1,  # bit 3 = 1
            4: 1,  # bit 4 = 1
            5: 0,  # bit 5 = 0
            6: 1,  # bit 6 = 1
            7: 0,  # bit 7 = 0
        },
    },
}

# 点位映射 (根据配置文件)
POINT_MAPPING = {
    1: {"register": 1, "bit": 0},  # 点位1 -> 寄存器1位0
    2: {"register": 1, "bit": 1},  # 点位2 -> 寄存器1位1
    3: {"register": 1, "bit": 2},  # 点位3 -> 寄存器1位2
    4: {"register": 1, "bit": 3},  # 点位4 -> 寄存器1位3
    5: {"register": 1, "bit": 4},  # 点位5 -> 寄存器1位4
    6: {"register": 1, "bit": 5},  # 点位6 -> 寄存器1位5
    7: {"register": 1, "bit": 6},  # 点位7 -> 寄存器1位6
    8: {"register": 1, "bit": 7},  # 点位8 -> 寄存器1位7
    9: {"register": 2, "bit": 0},  # 点位9 -> 寄存器2位0
    10: {"register": 2, "bit": 1},  # 点位10 -> 寄存器2位1
    11: {"register": 2, "bit": 2},  # 点位11 -> 寄存器2位2
    12: {"register": 2, "bit": 3},  # 点位12 -> 寄存器2位3
    13: {"register": 2, "bit": 4},  # 点位13 -> 寄存器2位4
    14: {"register": 2, "bit": 5},  # 点位14 -> 寄存器2位5
    15: {"register": 2, "bit": 6},  # 点位15 -> 寄存器2位6
    16: {"register": 2, "bit": 7},  # 点位16 -> 寄存器2位7
}


def print_expected_values():
    """打印期望的点位值"""
    print("=" * 60)
    print("期望的点位值 (基于模拟器设置)")
    print("=" * 60)

    print("\n寄存器1 (地址1): 0xA5 = 10100101")
    print("-" * 40)
    for point_id in range(1, 9):
        mapping = POINT_MAPPING[point_id]
        expected_value = SIMULATOR_VALUES[mapping["register"]]["bits"][mapping["bit"]]
        print(
            f"点位{point_id:2d} -> 寄存器{mapping['register']}位{mapping['bit']} = {expected_value}"
        )

    print("\n寄存器2 (地址2): 0x5A = 01011010")
    print("-" * 40)
    for point_id in range(9, 17):
        mapping = POINT_MAPPING[point_id]
        expected_value = SIMULATOR_VALUES[mapping["register"]]["bits"][mapping["bit"]]
        print(
            f"点位{point_id:2d} -> 寄存器{mapping['register']}位{mapping['bit']} = {expected_value}"
        )

    print("\n" + "=" * 60)
    print("请将此输出与comsrv读取的值对比")
    print("=" * 60)


if __name__ == "__main__":
    print_expected_values()
