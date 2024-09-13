/// Global script file for the SSGen full example
///
/// (c) theokrueger 2024
/// GPL-3.0 Licensed

console.log('javascript enabled for this webpage');

/* Replace the flavour text with a random selection */
const flavourTexts = [
	"Now with 50% more website for your website!",
	"No better price than free!",
	"GPL3-Pilled Libremaxxing",
	"For web undesigners",
	"It's hardly different from HTML!",
	":D",
	"Will not install Adobe Reader",
	"Documentation?? In this economy???",
	"A project with little purpose",
        "THE YAML CAML SPITS TOWARDS US ALL",
        "Made like a force-buy on round 2",
        "Does'nt come with spellcehck",
];

document.getElementById('flavour-text').innerHTML = flavourTexts[Math.floor(Math.random() * flavourTexts.length)];
