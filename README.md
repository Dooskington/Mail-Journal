# Mail Journal

A small utility application that helps you keep a small daily journal, while reminding you of your journal entry from 1 year prior.

Every day, the app will email you, asking for your thoughts on how the day went. Just send an email with your response, and that response will be stored. If you left a journal entry on that day during the previous year, this reminder email will also include that journal entry, giving you a nice reminder of how far you have come and allowing you to reflect.

I aimed to make this as lightweight as possible. It uses a very simple SQLite database for storing journal entries, and the only setup requirement is a mail server of some kind. It is very easy to create a new gmail account and link it by changing the SMTP and IMAP addresses in the config file.

## Running a Mail Journal server
TODO