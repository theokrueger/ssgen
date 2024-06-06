console.log('javascript enabled for this webpage');

const randomSubtitle = [
	"Now with 50% more website for your website!",
	"Because I didn't want to learn someone else's tools",
	"No better price than free!",
]


document.getElementById('jsreplace').innerHTML = randomSubtitle[Math.floor(Math.random() * randomSubtitle.length)];
