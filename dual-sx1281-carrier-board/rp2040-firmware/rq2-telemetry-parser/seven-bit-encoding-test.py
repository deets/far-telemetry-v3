def padded(bits):
    # The last 7 bit group is short,
    # so extend it to full length
    return bits + ['0'] * (7 - len(bits))

data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]

all_bits = []
for b in data:
    all_bits.extend(f"{b:08b}")
print("".join(all_bits))
seven_groups = []

index = 0
while True:
    seven = all_bits[index * 7: (index + 1) * 7]
    if seven:
        seven_groups.append(padded(seven))
    else:
        break
    index += 1

for (seven_bit, eight_bit) in zip(sum(seven_groups, []), all_bits):
    assert seven_bit == eight_bit


print("".join(f"X{''.join(sb)}" for sb in seven_groups))
print(" ".join(f"{int(''.join(sb), 2)}" for sb in seven_groups))
