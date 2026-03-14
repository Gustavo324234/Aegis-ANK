import re
regex = r'\[READ_FILE\("([^"]+)"\)\]'
stream = '[READ_FILE("src/main.rs")]'
match = re.search(regex, stream)
if match:
    print(f"Match: {match.group(1)}")
else:
    print("No match")
