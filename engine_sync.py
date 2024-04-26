import os
import subprocess
from datetime import datetime, timedelta
import random

def create_targeted_commits(start_date_str, end_date_str, repo_path, commits_per_day_range=(4, 6)):
    original_cwd = os.getcwd()
    try:
        os.chdir(repo_path)
        
        start_date = datetime.strptime(start_date_str, "%d-%m-%Y")
        end_date = datetime.strptime(end_date_str, "%d-%m-%Y")
        
        current_date = start_date
        total_days = (end_date - start_date).days + 1
        print(f"Generating targeted commits for {total_days} days...")
        
        for day_offset in range(total_days):
            current_date = start_date + timedelta(days=day_offset)
            num_commits = random.randint(*commits_per_day_range)
            
            for i in range(num_commits):
                hour = random.randint(9, 21)
                minute = random.randint(0, 59)
                second = random.randint(0, 59)
                commit_date = current_date.replace(hour=hour, minute=minute, second=second)
                date_str = commit_date.strftime("%Y-%m-%dT%H:%M:%S")
                
                with open("research_notes.md", "a") as f:
                    f.write(f"Research update at {date_str} - iteration {i+1}\n")
                
                subprocess.run(["git", "add", "research_notes.md"], check=True)
                env = os.environ.copy()
                env["GIT_AUTHOR_DATE"] = date_str
                env["GIT_COMMITTER_DATE"] = date_str
                
                subprocess.run(["git", "commit", "-m", f"research: data analysis checkpoint {date_str}"], env=env, check=True, capture_output=True)
            
            if day_offset % 10 == 0:
                print(f"Processed {day_offset} days...")
        
        print("Done!")
    finally:
        os.chdir(original_cwd)

if __name__ == "__main__":
    # Range: March 1, 2026 to May 31, 2026
    create_targeted_commits("01-03-2026", "31-05-2026", ".")
