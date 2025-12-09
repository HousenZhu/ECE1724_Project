import random

number = random.randint(1, 500)
attempts = 0

print("Welcome to the Number Guessing Game!")
print("I'm thinking of a number between 1 and 500. You have 10 attempts.")

while attempts < 10:
    guess = int(input("Enter your guess: "))
    attempts += 1
    if guess == number:
        print(f"Congratulations! You guessed the number in {attempts} attempts.")
        break
    elif guess < number:
        print("Too low. Try again.")
    else:
        print("Too high. Try again.")
else:
    print(f"Sorry, you've used all 10 attempts. The number was {number}.")