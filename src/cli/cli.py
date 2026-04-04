from argparse import ArgumentParser, Namespace


# Booting up


# Primary chat flow
# parser.add_argument('start', help="Start AEGIS program")
# parser.add_argument('start', help="Start AEGIS program")
# parser.add_argument('start', help="Start AEGIS program")

pos_args = ["chat", ""]

opt_args = {
    
}

if __name__ == "__main__":
    parser = ArgumentParser(description="Welcome to AEGIS, your privacy preserving AI chatbot!")
    
    parser.add_argument('start', help="Start AEGIS program")
    parser.add_argument('chat', help="One-shot prompt", type=str)
    parser.add_argument('-v', '--verbose', help="verbose description")

    args: Namespace = parser.parse_args()
