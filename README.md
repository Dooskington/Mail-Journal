<a href="https://github.com/Dooskington/Mail-Journal/">
    <img src="https://i.imgur.com/ej63msZ.png" alt="Mail Journal logo" title="Mail Journal" align="right" height="64" />
</a>

Mail Journal
============

A small utility application that helps you keep a small daily journal, while reminding you of your journal entry from 1 year prior.

Every day, the app will email you, asking for your thoughts on how the day went. Just send an email with your response, and that response will be stored. If you left a journal entry on that day during the previous year, this reminder email will also include that journal entry, giving you a nice reminder of how far you have come and allowing you to reflect.

![Mail Journal](https://i.imgur.com/XAxSNJS.png "Mail Journal")

My aim was to make this as lightweight as possible. It uses a very simple SQLite database for storing journal entries, and the only setup requirement is a mail server of some kind. It is very easy to create a new gmail account and link it by changing the SMTP and IMAP addresses in the config file.

## Configuration
When you run Mail Journal for the first time, a default config file will be created. You will need to input your own values for the following:

`target_email`: The email that you want to send the daily reminder to (your personal email).
`target_name`: Your name, for email purposes.
`db_filename`: The name of the database file that will be created/used.
`journal_email_smtp`: SMTP server domain for your journal email.
`journal_email_imap`: IMAP server domain for your journal email.
`journal_email`: The email address/username that Mail Journal will use.
`journal_email_password`: The password to the above email.
`utc_reminder_hour`: The hour during the day, in UTC, when you want to be reminded. Must be between 0 and 23 (inclusive).

## Hosting
Once you have filled out your config, you can just do a `cargo run` to run Mail Journal. My intended use was to leave the application running 24/7 on a remote server. If there is nothing for Mail Journal to do (no new journal entries to process and no reminder to be sent yet), then it will just sleep.

## Database
Mail Journal uses a very small SQLite database to store journal entries. If you want to modify the database at any time, you can do so easily by opening up the database file with a SQLite browser. You are also free to backup the database however you please.

## Issues
Please report any issues you may have. In particular, the IMAP/SMTP code may be slightly buggy since this project was my first time working with that kind of stuff.
