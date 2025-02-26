// Example JavaScript object with properties

const user = {
	id: 1,
	name: "John Smith",
	email: "john.smith@example.com",
	profile: {
		age: 30,
		occupation: "Software Engineer",
		skills: ["JavaScript", "TypeScript", "React", "Node.js"]
	},
	isActive: true,
	lastLogin: new Date("2023-01-01")
};

// Function to display user information
function displayUserInfo(user) {
	console.log(`User: ${user.name} (ID: ${user.id})`);
	console.log(`Email: ${user.email}`);
	console.log(`Occupation: ${user.profile.occupation}`);
	console.log(`Skills: ${user.profile.skills.join(", ")}`);
	console.log(`Active: ${user.isActive ? "Yes" : "No"}`);
	console.log(`Last Login: ${user.lastLogin.toLocaleDateString()}`);
}

// Call the function
displayUserInfo(user);
