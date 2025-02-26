package main

import "fmt"

// Person represents a person with various attributes
type Person struct {
	Name        string
	Age         int
	Email       string
	PhoneNumber string
	Address     Address
}

// Address represents a physical address
type Address struct {
	Street  string
	City    string
	State   string
	ZipCode string
	Country string
}

func main() {
	// Create a new person
	person := Person{
		Name:        "John Doe",
		Age:         30,
		Email:       "john.doe@example.com",
		PhoneNumber: "555-1234",
		Address: Address{
			Street:  "123 Main St",
			City:    "Anytown",
			State:   "CA",
			ZipCode: "12345",
			Country: "USA",
		},
	}

	// Print the person's information
	fmt.Printf("Name: %s\n", person.Name)
	fmt.Printf("Age: %d\n", person.Age)
	fmt.Printf("Email: %s\n", person.Email)
	fmt.Printf("Phone: %s\n", person.PhoneNumber)
	fmt.Printf("Address: %s, %s, %s %s, %s\n",
		person.Address.Street,
		person.Address.City,
		person.Address.State,
		person.Address.ZipCode,
		person.Address.Country)
}
