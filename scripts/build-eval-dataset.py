"""Source of truth for tests/fixtures/eval-dataset.json.

Each case has a human-written reference translation. References are ONE
acceptable rendering, not the only correct answer — evaluation is semantic
and human-reviewed (see docs/EVALUATION.md). `must_keep` lists terms that
must survive verbatim; `must_negate` marks sentences whose English must
contain a negation.
"""
import json, os

C = []
def add(cat, lang, src, ref, keep=(), neg=False, mode="accurate", note=""):
    C.append(dict(id=f"{cat[:4]}-{sum(1 for c in C if c['category']==cat)+1:02d}", category=cat,
                  expected_language=lang, mode=mode, source=src, reference=ref,
                  must_keep=list(keep), must_negate=neg, notes=note))

R, D, M, E = "romanized_nepali", "native_nepali", "mixed", "english"

# --- Everyday conversation
add("everyday", R, "Timi kasto chau?", "How are you?")
add("everyday", R, "Malai bhok lagyo.", "I'm hungry.")
add("everyday", R, "Aaja dherai jado cha.", "It's very cold today.")
add("everyday", R, "Ma bholi ghar janchu.", "I will go home tomorrow.")
add("everyday", R, "Khana khayau?", "Have you eaten?", note="Common greeting; literal 'did you eat food?'")
add("everyday", R, "Malai thaha chaina.", "I don't know.", neg=True)
add("everyday", R, "Ali bistarai bolnus na.", "Please speak a little more slowly.", note="'na' softens the request")
add("everyday", R, "Tapai kaha basnuhuncha?", "Where do you live?", note="Respectful register")
add("everyday", R, "Hijo pani pani paryo.", "It rained yesterday too.", note="First 'pani' = also, second 'pani' = water/rain; tests ambiguity")
add("everyday", R, "Mero phone ko battery sakiyo.", "My phone's battery has run out.")
add("everyday", R, "Ekchin parkhanus, ma aaudai chu.", "Please wait a moment, I'm coming.")
add("everyday", R, "Yo kati ho?", "How much is this?")
add("everyday", R, "Dhanyabad, dherai sahayog garnu bhayo.", "Thank you, you helped a lot.")
add("everyday", R, "Bus chuttyo, tyesaile dhilo bhayo.", "I missed the bus, so I was late.")
add("everyday", R, "Ama lai fon garna birse.", "I forgot to call Mom.")

# --- Workplace
add("workplace", M, "Maile hijo meeting ma technical problem ko barema kura gareko thiye tara solution clear bhayena.",
    "I discussed the technical problem during yesterday's meeting, but the solution was not clear.", keep=["meeting"], neg=True,
    note="Primary acceptance case")
add("workplace", M, "Maile aja project ko update pathauna sakina.", "I couldn't send the project update today.", neg=True)
add("workplace", M, "Bholi ko meeting 10 baje bata suru huncha.", "Tomorrow's meeting starts at 10 o'clock.", keep=["10"])
add("workplace", M, "Report Friday samma submit garnu parcha.", "The report has to be submitted by Friday.", keep=["Friday"])
add("workplace", M, "Manager le budget approve garnu bhayena.", "The manager did not approve the budget.", neg=True)
add("workplace", M, "Ma aaja work from home garchu.", "I'll work from home today.")
add("workplace", M, "Client sanga call ma kura bhayo, uniharu khusi chan.", "We talked with the client on the call; they are happy.")
add("workplace", M, "Deadline ali extend garna milcha?", "Could the deadline be extended a little?")
add("workplace", M, "Team ko sabai jana lai email pathaidinus.", "Please send the email to everyone on the team.")
add("workplace", M, "Maile tapai ko feedback anusar changes gareko chu.", "I have made the changes according to your feedback.")
add("workplace", M, "Aaja ma bimar chu, office aauna sakdina.", "I'm sick today and can't come to the office.", neg=True)
add("workplace", M, "Invoice ma galti cha jasto lagcha.", "It seems there is a mistake in the invoice.", note="Keep hedge 'seems'")
add("workplace", M, "Hami next week bata naya process follow garchau.", "We will follow the new process from next week.")
add("workplace", M, "Interview kasto bhayo? Result kahile aaucha?", "How did the interview go? When will the result come?")
add("workplace", M, "Sayad bholi samma kaam sakincha.", "The work will probably be finished by tomorrow.", note="'sayad' = maybe/probably; must keep uncertainty")

# --- Technical
add("technical", M, "Maile Kubernetes cluster deploy gare.", "I deployed the Kubernetes cluster.", keep=["Kubernetes"])
add("technical", R, "Yo error kina aayo?", "Why did this error occur?")
add("technical", M, "Deployment complete bhayo tara pods ready chainan.", "The deployment completed, but the pods are not ready.", keep=["pods"], neg=True)
add("technical", M, "AWS ma EC2 instance start bhayena.", "The EC2 instance did not start on AWS.", keep=["AWS", "EC2"], neg=True)
add("technical", M, "Database connection timeout bhairacha.", "The database connection keeps timing out.", note="'bhairacha' = continuous/repeated")
add("technical", M, "Login garda 500 error aaucha.", "A 500 error appears when logging in.", keep=["500"])
add("technical", M, "Maile bug fix gareko chu, PR review garidinus.", "I have fixed the bug; please review the PR.", keep=["PR"])
add("technical", M, "Production ma rollback garnu paryo.", "We had to roll back in production.")
add("technical", M, "Docker image build garda dependency missing bhanyo.", "When building the Docker image, it said a dependency is missing.", keep=["Docker"])
add("technical", M, "Yo API le JSON response didaina.", "This API does not return a JSON response.", keep=["API", "JSON"], neg=True)
add("technical", M, "CI pipeline feri fail bhayo.", "The CI pipeline failed again.", keep=["CI"], neg=True, note="'fail' expresses negative outcome")
add("technical", M, "Terraform apply garnu agadi plan check garnus.", "Check the plan before running terraform apply.", keep=["Terraform", "apply", "plan"])
add("technical", M, "Memory usage 90 percent bhanda mathi gayo.", "Memory usage went above 90 percent.", keep=["90"])
add("technical", M, "Yo function le null return garcha kahile kahi.", "This function sometimes returns null.", keep=["null"])
add("technical", M, "kubectl get pods chalauda kehi pani dekhaudaina.", "Running kubectl get pods shows nothing.", keep=["kubectl get pods"], neg=True)
add("technical", M, "Staging environment ma test garera matra merge garnus.", "Only merge after testing in the staging environment.", keep=["Staging"])
add("technical", M, "S3 bucket ko permission public chaina ni?", "The S3 bucket's permissions aren't public, right?", keep=["S3"], neg=True)
add("technical", M, "Load balancer le traffic distribute gardaina jasto cha.", "It seems the load balancer is not distributing traffic.", neg=True, note="Keep hedge")
add("technical", M, "Git conflict resolve garna sakina, help garnus.", "I couldn't resolve the Git conflict; please help.", keep=["Git"], neg=True)
add("technical", M, "Yo version 2.3.1 ma bug cha, 2.3.0 ma thiyena.", "There is a bug in version 2.3.1; it wasn't in 2.3.0.", keep=["2.3.1", "2.3.0"], neg=True)

# --- Customer support
add("support", M, "Mero account login hudaina, password reset garna milcha?", "I can't log in to my account; can I reset my password?", neg=True)
add("support", M, "Order gareko tin din bhayo tara ajhai deliver bhayena.", "It has been three days since I ordered, but it still hasn't been delivered.", neg=True)
add("support", M, "Refund kahile samma aaucha?", "By when will the refund arrive?")
add("support", M, "App update gare pachi crash bhairacha.", "The app keeps crashing after I updated it.")
add("support", M, "Hamro team le tapai ko issue herdai cha, chadai jankari dinchhau.", "Our team is looking into your issue; we will update you soon.")

# --- Academic
add("academic", M, "Mero assignment ko deadline bholi ho.", "My assignment's deadline is tomorrow.", keep=["assignment"])
add("academic", M, "Yo research le climate change ko asar Nepal ma dekhaucha.", "This research shows the impact of climate change in Nepal.")
add("academic", M, "Maile literature review ko pahilo draft lekhe.", "I wrote the first draft of the literature review.")
add("academic", M, "Data ali kam bhayeko le result pakka bhanna sakidaina.", "Because the data is limited, the result cannot be stated with certainty.", neg=True, note="Must not strengthen the claim")
add("academic", M, "Professor le thesis ko methodology change garna bhannu bhayo.", "The professor told me to change the thesis methodology.")
add("academic", M, "Survey ma 120 jana le bhag line.", "120 people took part in the survey.", keep=["120"], note="'line' here = took (part); ambiguous with English 'line'")
add("academic", M, "Yo nishkarsha sabai paristhiti ma lagu nahuna sakcha.", "This conclusion may not apply in all situations.", neg=True)
add("academic", M, "Class ma presentation dina dar lagcha.", "I'm nervous about giving presentations in class.")

# --- Informal
add("informal", R, "K cha bro?", "What's up, bro?")
add("informal", R, "Ekdam ramro bhayo yaar!", "That was really great, man!")
add("informal", R, "Malai ta jhyau lagyo.", "I got really bored.", note="'jhyau' = boredom/annoyance")
add("informal", R, "Ke garne, yestai ho life.", "What can you do, that's life.")
add("informal", R, "Oe, kata hidis?", "Hey, where did you go?", note="Very informal (ta-form)")
add("informal", R, "Sachchi? Pakka ho?", "Really? Are you sure?")
add("informal", R, "Thik cha, pachi bhetaula.", "Okay, see you later.")
add("informal", R, "Tyo movie ta bakwas rahecha.", "That movie turned out to be rubbish.", note="'rahecha' = discovered/turned out")
add("informal", R, "La la, chhito aija.", "Okay okay, come quickly.")
add("informal", R, "Malai yo kura man pardaina.", "I don't like this.", neg=True)

# --- Devanagari
add("devanagari", D, "मैले हिजो मिटिङमा प्राविधिक समस्याको बारेमा कुरा गरेको थिएँ, तर समाधान स्पष्ट भएन।",
    "I discussed the technical problem during yesterday's meeting, but the solution was not clear.", neg=True)
add("devanagari", D, "तपाईंलाई कस्तो छ?", "How are you?")
add("devanagari", D, "म भोलि काठमाडौं जाँदैछु।", "I am going to Kathmandu tomorrow.", note="Place name should be rendered as Kathmandu")
add("devanagari", D, "यो काम आज सकिँदैन।", "This work cannot be finished today.", neg=True)
add("devanagari", D, "कृपया ढोका बन्द गर्नुहोस्।", "Please close the door.")
add("devanagari", D, "उहाँ अहिले बैठकमा हुनुहुन्छ।", "He/She is in a meeting right now.", note="Gender not marked in Nepali; English must not invent gender (he/she or they)")
add("devanagari", D, "मलाई यो कुरा थाहा थिएन।", "I didn't know this.", neg=True)
add("devanagari", D, "हामीले परियोजना समयमै सक्यौं।", "We finished the project on time.")
add("devanagari", D, "पानी पर्ला जस्तो छ।", "It looks like it might rain.", note="Keep uncertainty")
add("devanagari", D, "विद्यार्थीहरूले परीक्षा राम्रोसँग दिए।", "The students did well in the exam.")
add("devanagari", M, "मेरो Kubernetes cluster चलेन।", "My Kubernetes cluster didn't run.", keep=["Kubernetes"], neg=True)
add("devanagari", D, "उनले भने कि उनी आउँदैनन्।", "They said that they would not come.", neg=True, note="Reported speech; respectful 'unle' — gender unmarked")

# --- Tense & negation
add("tense", R, "Ma khana khanchu.", "I eat / I will eat food.", note="Nepali simple present often covers future")
add("tense", R, "Maile khana khaye.", "I ate.")
add("tense", R, "Maile khana khaisake.", "I have already eaten.")
add("tense", R, "Ma khana khadai chu.", "I am eating.")
add("tense", R, "Maile khana khayeko thiye.", "I had eaten.")
add("tense", R, "Ma khana khadina.", "I don't eat / I won't eat.", neg=True)
add("tense", R, "Maile khana khayina.", "I didn't eat.", neg=True)
add("tense", R, "Usle aajhai phone uthayena.", "He/She still hasn't picked up the phone.", neg=True)
add("tense", R, "Tyo kaam kahile pani bhayena.", "That work was never done.", neg=True)
add("tense", R, "Kasaile pani bhanena.", "Nobody said anything.", neg=True)
add("tense", R, "Ma janu pardaina.", "I don't have to go.", neg=True, note="Absence of obligation, not prohibition")
add("tense", R, "Timi najau.", "Don't go.", neg=True, note="Prohibitive na- prefix")

# --- Ambiguous
add("ambiguous", R, "Ma ghar ma chu.", "I am at home.", note="'Ma' = I, 'ma' = at; both in one sentence")
add("ambiguous", R, "Uslai kura lagyo.", "He/She was hurt by what was said.", note="'kura lagnu' idiom = take offence; literal = words struck")
add("ambiguous", R, "Ko aayo?", "Who came?", note="'ko' = who, not possessive")
add("ambiguous", R, "Tyo manche ko ghar thulo cha.", "That person's house is big.", note="'ko' = possessive here")
add("ambiguous", R, "Malai dar lagyo.", "I got scared.", note="'dar' = fear; could be misread as English 'dare'")
add("ambiguous", M, "Hami sabai thik chau, chinta nagarnus.", "We're all fine, don't worry.", neg=True)

# --- Grammar correction (English input)
add("grammar", E, "I has send the report yesterday.", "I sent the report yesterday.", mode="grammar")
add("grammar", E, "He don't know about the meeting.", "He doesn't know about the meeting.", mode="grammar", neg=True)
add("grammar", E, "The pods is not ready because deployment failed.", "The pods are not ready because the deployment failed.", keep=["pods"], mode="grammar", neg=True)
add("grammar", E, "Please do the needful and revert back.", "Please take the necessary action and reply.", mode="grammar", note="Regional idiom; minimal change acceptable")
add("grammar", E, "Me and my friend goes to college everyday.", "My friend and I go to college every day.", mode="grammar")

# --- Long paragraphs
add("long", M,
    "Hijo hamro team le production ma naya version deploy garyo. Deploy gare pachi kehi users le login garna sakenan. "
    "Hamile logs herda authentication service ma timeout dekhiyo. Ahile temporary fix lagayeko chau tara root cause ajhai pattalagaunu baki cha.",
    "Yesterday our team deployed a new version to production. After the deployment, some users could not log in. "
    "When we checked the logs, we saw timeouts in the authentication service. We have applied a temporary fix for now, but the root cause still needs to be identified.",
    keep=["production", "authentication service"], neg=True)
add("long", M,
    "Ma university ma MBA padhdai chu. Kaam ra padhai sangai garna garo cha, tara ma time management sikdai chu. "
    "Yo semester ma tin ota assignment ra euta group project cha.",
    "I am studying for an MBA at university. Doing work and studies together is difficult, but I am learning time management. "
    "This semester there are three assignments and one group project.",
    keep=["MBA"], note="Progressive aspect padhdai chu; sikdai chu")
add("long", R,
    "Hamro gaun ma bato banna thaleko chha. Pahile bazaar jana dui ghanta lagthyo, aba bus chalna thalepachhi aadha ghanta ma pugincha. "
    "Tara barsat ma bato bigrincha ki bhanne chinta chha.",
    "A road has started being built in our village. It used to take two hours to go to the market; now that buses have started running, you get there in half an hour. "
    "But there is worry that the road may get damaged during the monsoon.",
    note="Habitual past 'lagthyo'; hedge 'ki bhanne chinta'")

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tests", "fixtures", "eval-dataset.json")
json.dump({"version": 1, "description": "DigitsNepal Nepali-English evaluation set. References are human-written, one acceptable rendering each; judge semantically.", "cases": C},
          open(out, "w"), ensure_ascii=False, indent=2)
print(len(C), "cases")
